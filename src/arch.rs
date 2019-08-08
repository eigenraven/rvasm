
use std::collections::HashMap;
use smallvec::SmallVec;

/*trait Register where Self: std::marker::Sized {
    fn name(&self) -> &'static str;
    fn abi_name(&self) -> &'static str;
    fn from_name(name: &str) -> Option<Self>;
    fn number(&self) -> i32;
    fn from_number(n: i32) -> Option<Self>;
}*/

// Values' [last:first] bits map onto instructions' [first+vlast-vfirst:first] bits.
#[derive(Copy, Clone, Debug, Default)]
struct BitRangeMap {
    value_last: i32,
    value_first: i32,
    instruction_first: i32,
}

#[derive(Clone, Debug, Default)]
struct InstructionField {
    name: String,
    /// Total length of the value in bits
    length: i32,
    encoding: SmallVec<[BitRangeMap; 2]>,
}

#[derive(Clone, Debug, Default)]
struct InstructionFormat {
    name: String,
    fields: SmallVec<[InstructionField; 8]>,
}

#[derive(Debug, Default)]
struct RiscVAbi {
    // Meta
    loaded_names: Vec<String>,
    loaded_codes: Vec<String>,
    loaded_specs: Vec<String>,
    // Consts
    consts: HashMap<String, i32>,
    // Registers
    register_names: HashMap<i32, Vec<String>>,
    register_name_lookup: HashMap<String, i32>,
    register_sizes: HashMap<i32, i32>,
    // Instruction formats
    instruction_formats: Vec<InstructionFormat>
}


