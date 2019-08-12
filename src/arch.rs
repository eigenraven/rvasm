use smallvec::SmallVec;
use std::collections::HashMap;
use toml;

#[derive(Clone, Debug, Default)]
pub struct Register {
    pub index: i32,
    pub names: Vec<String>,
    pub size_in_bits: i32,
}

impl Register {
    pub fn new(index: i32) -> Self {
        Self {
            index,
            ..Default::default()
        }
    }

    pub fn get_main_name(&self) -> Option<&str> {
        self.names.get(0).map(|x| x.as_ref())
    }

    pub fn get_abi_name(&self) -> Option<&str> {
        self.names
            .get(1)
            .or_else(|| self.names.get(0))
            .map(|x| x.as_ref())
    }
}

// Values' [last:first] bits map onto instructions' [first+vlast-vfirst:first] bits.
#[derive(Copy, Clone, Debug, Default)]
pub struct BitRangeMap {
    pub value_last: i32,
    pub value_first: i32,
    pub instruction_first: i32,
}

impl BitRangeMap {
    pub fn new(value_last: i32, value_first: i32, instruction_first: i32) -> Self {
        Self {
            value_last,
            value_first,
            instruction_first,
        }
    }

    pub fn instruction_last(&self) -> i32 {
        self.instruction_first + self.value_last - self.value_first
    }

    pub fn value_bitmask(&self) -> u64 {
        let value_len = self.value_last - self.value_first + 1;
        ((1 << value_len) - 1) << self.value_first
    }

    pub fn encode_into(&self, bytes: &mut [u8], value: u64) {
        let mut enc_value = (value & self.value_bitmask()) >> self.value_first;
        let mut enc_mask = self.value_bitmask() >> self.value_first;
        let mut instr_byte = self.instruction_first as usize / 8;
        enc_value <<= self.instruction_first as usize % 8;
        enc_mask <<= self.instruction_first as usize % 8;
        while enc_mask != 0 {
            let bmask = (enc_mask & 0xff) as u8;
            let bval = (enc_value & 0xff) as u8;
            // zero out the bits to encode
            bytes[instr_byte] &= !bmask;
            // encode bits
            bytes[instr_byte] |= bval;
            // move on to next 8 bits
            enc_mask >>= 8;
            enc_value >>= 8;
            instr_byte += 1;
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum FieldType {
    Register,
    Value,
}

#[derive(Clone, Debug)]
pub struct InstructionField {
    pub name: String,
    pub vtype: FieldType,
    /// Total length of the value in bits
    pub length: i32,
    pub encoding: SmallVec<[BitRangeMap; 2]>,
}

impl InstructionField {
    fn calculate_last_encoded_bit_index(&self) -> i32 {
        self.encoding
            .iter()
            .map(|e| e.instruction_last())
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct InstructionFormat {
    pub name: String,
    pub fields: SmallVec<[InstructionField; 8]>,
    pub ilen: usize,
}

impl InstructionFormat {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    fn calculate_last_encoded_bit_index(&self) -> i32 {
        self.fields
            .iter()
            .map(|e| e.calculate_last_encoded_bit_index())
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct InstructionDefinition {
    pub name: String,
    pub format_idx: usize,
    /// Indices into InstructionFormat.fields
    pub args: Vec<usize>,
    /// Indices into InstructionFormat.fields paired with assigned values
    pub fields: Vec<(usize, u64)>,
}

impl InstructionDefinition {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn get_format<'spec>(&self, spec: &'spec RiscVSpec) -> &'spec InstructionFormat {
        spec.get_instruction_format(self.format_idx).unwrap()
    }

    pub fn encode_into(
        &self,
        bytes: &mut [u8],
        spec: &RiscVSpec,
        argvals: &[u64],
    ) -> Result<(), ()> {
        assert_eq!(argvals.len(), self.args.len());
        let fmt = self.get_format(spec);
        for (fldid, fldval) in self.fields.iter() {
            let fld: &InstructionField = &fmt.fields[*fldid];
            fld.encoding
                .iter()
                .for_each(|e| e.encode_into(bytes, *fldval));
        }
        for (argid, argval) in self.args.iter().zip(argvals) {
            let arg: &InstructionField = &fmt.fields[*argid];
            arg.encoding
                .iter()
                .for_each(|e| e.encode_into(bytes, *argval));
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct RiscVSpec {
    // Meta
    loaded_names: Vec<String>,
    loaded_codes: Vec<String>,
    loaded_specs: Vec<String>,
    // Consts
    consts: HashMap<String, u64>,
    // Registers
    registers: HashMap<i32, Register>,
    register_name_lookup: HashMap<String, i32>,
    // Instruction formats
    instruction_formats: Vec<InstructionFormat>,
    // Instructions
    instructions: Vec<InstructionDefinition>,
    instruction_name_lookup: HashMap<String, usize>,
}

pub struct AbiFileInfo<'a> {
    pub name: &'a str,
    pub code: &'a str,
    pub spec: &'a str,
}

// Main functionality
impl RiscVSpec {
    pub fn get_loaded_abis(&self) -> Vec<AbiFileInfo> {
        let mut v = Vec::new();
        assert_eq!(self.loaded_names.len(), self.loaded_codes.len());
        assert_eq!(self.loaded_names.len(), self.loaded_specs.len());
        for ((name, code), spec) in self
            .loaded_names
            .iter()
            .zip(self.loaded_codes.iter())
            .zip(self.loaded_specs.iter())
        {
            v.push(AbiFileInfo { name, code, spec });
        }
        v
    }

    // Consts

    pub fn get_const(&self, name: &str) -> Option<u64> {
        self.consts.get(name).copied()
    }

    // Registers

    pub fn get_register(&self, rnum: i32) -> Option<&Register> {
        self.registers.get(&rnum)
    }

    pub fn get_register_by_name(&self, rname: &str) -> Option<&Register> {
        self.register_name_lookup
            .get(rname)
            .and_then(|i| self.get_register(*i))
    }

    pub fn get_all_registers(&self) -> &HashMap<i32, Register> {
        &self.registers
    }

    // Instruction Formats

    pub fn get_instruction_format(&self, index: usize) -> Option<&InstructionFormat> {
        self.instruction_formats.get(index)
    }

    pub fn get_instruction_format_by_name(&self, name: &str) -> Option<&InstructionFormat> {
        self.instruction_name_lookup
            .get(name)
            .and_then(|i| self.get_instruction_format(*i))
    }

    pub fn get_all_instruction_formats(&self) -> &[InstructionFormat] {
        &self.instruction_formats
    }

    // Instructions

    pub fn get_instruction(&self, index: usize) -> Option<&InstructionDefinition> {
        self.instructions.get(index)
    }

    /// Automatically converts name to lowercase
    pub fn get_instruction_by_name(&self, name: &str) -> Option<&InstructionDefinition> {
        self.instruction_name_lookup
            .get(&name.to_ascii_lowercase())
            .and_then(|i| self.get_instruction(*i))
    }

    pub fn get_all_instructions(&self) -> &[InstructionDefinition] {
        &self.instructions
    }
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

// Creation & Parsing
impl RiscVSpec {
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
        consts: &HashMap<String, u64>,
        key: String,
        v: &toml::Value,
    ) -> Result<i64, LoadError> {
        if let Some(i) = v.as_integer() {
            Ok(i)
        } else if let Some(s) = v.as_str() {
            consts
                .get(s)
                .ok_or_else(|| LoadError::ConstNotFound(s.to_owned()))
                .map(|x| *x as i64)
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
                let intvalue = Self::toml_int(&self.consts, format!("consts.{}", k), v)? as u64;
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
                    self.registers
                        .entry(number)
                        .or_insert_with(|| Register::new(number));
                    let mut newnames = Vec::new();
                    for name in names.iter() {
                        let name = name.as_str().ok_or_else(|| {
                            LoadError::BadType(format!("registers.names.{} element", number))
                        })?;
                        newnames.push(name.to_owned());
                    }
                    self.registers.get_mut(&number).unwrap().names = newnames;
                }
            }
            if let Some(register_lengths) = registers.get("lengths") {
                let register_lengths = register_lengths
                    .as_table()
                    .ok_or_else(|| BadType("registers.lengths"))?;
                for (number, length) in register_lengths.iter() {
                    let number: i32 = number.parse().map_err(|_| {
                        LoadError::BadType(format!("registers.lengths.{} key", number))
                    })?;
                    let length = Self::toml_int(
                        &self.consts,
                        format!("registers.lengths.{} value", number),
                        length,
                    )? as i32;
                    self.registers
                        .entry(number)
                        .or_insert_with(|| Register::new(number));
                    self.registers.get_mut(&number).unwrap().size_in_bits = length;
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
                    let mut fld = InstructionField {
                        name: fldname.to_owned(),
                        vtype: FieldType::Value,
                        length: 0,
                        encoding: Default::default(),
                    };
                    let fldtype = fldtable
                        .get("type")
                        .ok_or_else(|| {
                            LoadError::MissingNode(format!(
                                "instruction_formats.{}.{}.type",
                                fmtname, fldname
                            ))
                        })?
                        .as_str()
                        .ok_or_else(|| {
                            LoadError::BadType(format!(
                                "instruction_formats.{}.{}.type",
                                fmtname, fldname
                            ))
                        })?;
                    match fldtype {
                        "value" => {
                            fld.vtype = FieldType::Value;
                        }
                        "register" => {
                            fld.vtype = FieldType::Register;
                        }
                        _ => {
                            return Err(LoadError::BadType(format!(
                                "instruction_formats.{}.{}.type",
                                fmtname, fldname
                            )));
                        }
                    }
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
                fmt.ilen = fmt.calculate_last_encoded_bit_index() as usize + 1;
                self.instruction_formats.push(fmt);
            }
        }

        // parse instructions
        if let Some(instructions) = instructions {
            let instructions = instructions
                .as_table()
                .ok_or_else(|| BadType("instructions"))?;
            for (iname, itable) in instructions.iter() {
                let iname = iname.to_ascii_lowercase();
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
                    insn.fields.push((fi, fv as u64));
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

        // update register name mapping
        self.register_name_lookup.clear();
        for (num, reg) in self.registers.iter() {
            for name in reg.names.iter() {
                self.register_name_lookup.insert(name.to_owned(), *num);
            }
        }
        Ok(())
    }
}
