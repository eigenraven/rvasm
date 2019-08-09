use smallvec::SmallVec;
use std::collections::HashMap;
use toml;

/*trait Register where Self: std::marker::Sized {
    fn name(&self) -> &'static str;
    fn abi_name(&self) -> &'static str;
    fn from_name(name: &str) -> Option<Self>;
    fn number(&self) -> i32;
    fn from_number(n: i32) -> Option<Self>;
}*/

// Values' [last:first] bits map onto instructions' [first+vlast-vfirst:first] bits.
#[derive(Copy, Clone, Debug, Default)]
pub struct BitRangeMap {
    pub value_last: i32,
    pub value_first: i32,
    pub instruction_first: i32,
}

impl BitRangeMap {
    fn new(value_last: i32, value_first: i32, instruction_first: i32) -> Self {
        Self {
            value_last,
            value_first,
            instruction_first,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InstructionField {
    pub name: String,
    /// Total length of the value in bits
    pub length: i32,
    pub encoding: SmallVec<[BitRangeMap; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct InstructionFormat {
    pub name: String,
    pub fields: SmallVec<[InstructionField; 8]>,
}

impl InstructionFormat {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InstructionDefinition {
    pub name: String,
    pub format_idx: usize,
    /// Indices into InstructionFormat.fields
    pub args: Vec<usize>,
    /// Indices into InstructionFormat.fields paired with assigned values
    pub fields: Vec<(usize, i64)>,
}

impl InstructionDefinition {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default)]
pub struct RiscVAbi {
    // Meta
    loaded_names: Vec<String>,
    loaded_codes: Vec<String>,
    loaded_specs: Vec<String>,
    // Consts
    consts: HashMap<String, i64>,
    // Registers
    register_names: HashMap<i32, Vec<String>>,
    register_name_lookup: HashMap<String, i32>,
    register_sizes: HashMap<i32, i32>,
    // Instruction formats
    instruction_formats: Vec<InstructionFormat>,
    // Instructions
    instructions: Vec<InstructionDefinition>,
    instruction_name_lookup: HashMap<String, usize>,
}

#[derive(Clone, Debug)]
pub enum LoadError {
    MalformedTOML,
    RequirementNotFound(String),
    ConstNotFound(String),
    MissingNode(String),
    BadType(String),
    DuplicateInstruction(String),
    BadInstructionFormat(String),
}

impl RiscVAbi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_single_cfg_string(&mut self, content: &str) -> Result<(), LoadError> {
        use toml::Value;
        let val = content
            .parse::<Value>()
            .map_err(|_| LoadError::MalformedTOML)?;
        self.load_single_toml(val)
    }

    /// Load integer or search in consts if a string
    fn toml_int(
        consts: &HashMap<String, i64>,
        key: String,
        v: &toml::Value,
    ) -> Result<i64, LoadError> {
        if let Some(i) = v.as_integer() {
            Ok(i)
        } else if let Some(s) = v.as_str() {
            consts
                .get(s)
                .ok_or_else(|| LoadError::ConstNotFound(s.to_owned()))
                .map(|x| *x)
        } else {
            Err(LoadError::BadType(key))
        }
    }

    fn load_single_toml(&mut self, doc: toml::Value) -> Result<(), LoadError> {
        #[allow(non_snake_case)]
        let MissingNode = |s: &'static str| LoadError::MissingNode(s.to_owned());
        #[allow(non_snake_case)]
        let BadType = |s: &'static str| LoadError::BadType(s.to_owned());

        let meta = doc.get("meta").ok_or_else(|| MissingNode("meta"))?;
        let consts = doc.get("consts");
        let registers = doc.get("registers");
        let instruction_formats = doc.get("instruction_formats");
        let instructions = doc.get("instructions");

        self.loaded_names.push(
            meta.get("name")
                .ok_or_else(|| MissingNode("meta.name"))?
                .as_str()
                .ok_or_else(|| BadType("meta.name"))?
                .to_owned(),
        );
        self.loaded_codes.push(
            meta.get("code")
                .ok_or_else(|| MissingNode("meta.code"))?
                .as_str()
                .ok_or_else(|| BadType("meta.code"))?
                .to_owned(),
        );
        self.loaded_specs.push(
            meta.get("spec")
                .ok_or_else(|| MissingNode("meta.spec"))?
                .as_str()
                .ok_or_else(|| BadType("meta.spec"))?
                .to_owned(),
        );

        // validate requirements
        let requires = meta.get("requires");
        if let Some(requires) = requires {
            let list = requires
                .as_array()
                .ok_or_else(|| BadType("meta.requires"))?;
            for rq in list.iter() {
                let code = rq.as_str().ok_or_else(|| BadType("meta.requires item"))?;
                if !self.loaded_codes.iter().any(|s| s == code) {
                    return Err(LoadError::RequirementNotFound(code.to_owned()));
                }
            }
        }

        // parse consts
        if let Some(consts) = consts {
            let consts = consts.as_table().ok_or_else(|| BadType("consts"))?;
            for (k, v) in consts.iter() {
                let intvalue = Self::toml_int(&self.consts, format!("consts.{}", k), v)?;
                self.consts.insert(k.to_owned(), intvalue);
            }
        }

        // parse registers
        if let Some(registers) = registers {
            let registers = registers.as_table().ok_or_else(|| BadType("registers"))?;
            if let Some(register_names) = registers.get("names") {
                let register_names = register_names
                    .as_table()
                    .ok_or_else(|| BadType("registers.names"))?;
                for (number, names) in register_names.iter() {
                    let number: i32 = number.parse().map_err(|_| {
                        LoadError::BadType(format!("registers.names.{} key", number))
                    })?;
                    let names = names.as_array().ok_or_else(|| {
                        LoadError::BadType(format!("registers.names.{} value", number))
                    })?;
                    let mut newnames = Vec::new();
                    for name in names.iter() {
                        let name = name.as_str().ok_or_else(|| {
                            LoadError::BadType(format!("registers.names.{} element", number))
                        })?;
                        newnames.push(name.to_owned());
                        // FIXME: Handling name overrides/removals?
                        self.register_name_lookup.insert(name.to_owned(), number);
                    }
                    self.register_names.insert(number, newnames);
                }
            }
        }

        // parse instruction_formats
        if let Some(instruction_formats) = instruction_formats {
            let instruction_formats = instruction_formats
                .as_table()
                .ok_or_else(|| BadType("instruction_formats"))?;
            for (fmtname, fmttable) in instruction_formats.iter() {
                let fmttable = fmttable.as_table().ok_or_else(|| {
                    LoadError::BadType(format!("instruction_formats.{}", fmtname))
                })?;
                let mut fmt = InstructionFormat::new(fmtname.to_owned());
                for (fldname, fldtable) in fmttable.iter() {
                    let fldtable = fldtable.as_table().ok_or_else(|| {
                        LoadError::BadType(format!("instruction_formats.{}.{}", fmtname, fldname))
                    })?;
                    let mut fld = InstructionField::default();
                    fld.name = fldname.to_owned();
                    fld.length = Self::toml_int(
                        &self.consts,
                        format!("instruction_formats.{}.{}.length", fmtname, fldname),
                        fldtable.get("length").ok_or_else(|| {
                            LoadError::MissingNode(format!(
                                "instruction_formats.{}.{}.length",
                                fmtname, fldname
                            ))
                        })?,
                    )? as i32;
                    let fldencoding = fldtable
                        .get("encoding")
                        .ok_or_else(|| {
                            LoadError::MissingNode(format!(
                                "instruction_formats.{}.{}.encoding",
                                fmtname, fldname
                            ))
                        })?
                        .as_array()
                        .ok_or_else(|| {
                            LoadError::BadType(format!(
                                "instruction_formats.{}.{}.encoding",
                                fmtname, fldname
                            ))
                        })?;
                    for val in fldencoding.iter() {
                        let subenc = val.as_array().ok_or_else(|| {
                            LoadError::BadType(format!(
                                "instruction_formats.{}.{}.encoding[] element",
                                fmtname, fldname
                            ))
                        })?;
                        if subenc.len() != 3 {
                            return Err(LoadError::BadType(format!(
                                "instruction_formats.{}.{}.encoding[][] length (must be 3)",
                                fmtname, fldname
                            )));
                        }
                        let vend = Self::toml_int(
                            &self.consts,
                            format!("instruction_formats.{}.{}.encoding[][]", fmtname, fldname),
                            &subenc[0],
                        )? as i32;
                        let vbegin = Self::toml_int(
                            &self.consts,
                            format!("instruction_formats.{}.{}.encoding[][]", fmtname, fldname),
                            &subenc[1],
                        )? as i32;
                        let ibegin = Self::toml_int(
                            &self.consts,
                            format!("instruction_formats.{}.{}.encoding[][]", fmtname, fldname),
                            &subenc[2],
                        )? as i32;

                        fld.encoding.push(BitRangeMap::new(vend, vbegin, ibegin));
                    }
                    fmt.fields.push(fld);
                }
                self.instruction_formats.push(fmt);
            }
        }

        // parse instructions
        if let Some(instructions) = instructions {
            let instructions = instructions
                .as_table()
                .ok_or_else(|| BadType("instructions"))?;
            for (iname, itable) in instructions.iter() {
                let iname = iname.to_lowercase();
                let itable = itable
                    .as_table()
                    .ok_or_else(|| LoadError::BadType(format!("instructions.{}", iname)))?;

                let iformat = itable
                    .get("format")
                    .ok_or_else(|| {
                        LoadError::MissingNode(format!("instructions.{}.format", iname))
                    })?
                    .as_str()
                    .ok_or_else(|| LoadError::BadType(format!("instructions.{}.format", iname)))?;

                let iargs = itable
                    .get("args")
                    .ok_or_else(|| LoadError::MissingNode(format!("instructions.{}.args", iname)))?
                    .as_array()
                    .ok_or_else(|| LoadError::BadType(format!("instructions.{}.args", iname)))?;

                let ifields = itable
                    .get("fields")
                    .ok_or_else(|| {
                        LoadError::MissingNode(format!("instructions.{}.fields", iname))
                    })?
                    .as_table()
                    .ok_or_else(|| LoadError::BadType(format!("instructions.{}.fields", iname)))?;

                let mut insn = InstructionDefinition::new(iname.clone());

                insn.format_idx = self
                    .instruction_formats
                    .iter()
                    .position(|x| x.name == iformat)
                    .ok_or_else(|| {
                        LoadError::BadInstructionFormat(format!("instructions.{}.format", iname))
                    })?;
                let fmt = &self.instruction_formats[insn.format_idx];

                for argv in iargs.iter() {
                    let argv = argv.as_str().ok_or_else(|| {
                        LoadError::BadType(format!("instructions.{}.args[] item", iname))
                    })?;
                    insn.args
                        .push(
                            fmt.fields
                                .iter()
                                .position(|x| x.name == argv)
                                .ok_or_else(|| {
                                    LoadError::BadInstructionFormat(format!(
                                        "instructions.{}.args[{}]",
                                        iname, argv
                                    ))
                                })?,
                        );
                }

                for (fname, fv) in ifields.iter() {
                    let fv = Self::toml_int(
                        &self.consts,
                        format!("instructions.{}.fields[{}]", iname, fname),
                        fv,
                    )?;
                    let fi = fmt
                        .fields
                        .iter()
                        .position(|x| x.name == fname.as_ref())
                        .ok_or_else(|| {
                            LoadError::BadInstructionFormat(format!(
                                "instructions.{}.fields[{}]",
                                iname, fname
                            ))
                        })?;
                    insn.fields.push((fi, fv));
                }

                if self
                    .instruction_name_lookup
                    .insert(iname.clone(), self.instructions.len())
                    .is_some()
                {
                    return Err(LoadError::DuplicateInstruction(iname.clone()));
                }
                self.instructions.push(insn);
            }
        }

        Ok(())
    }
}
