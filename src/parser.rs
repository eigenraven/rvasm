use crate::arch;

#[allow(ellipsis_inclusive_range_patterns)]
#[allow(clippy::all)]
mod grammar {
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

#[derive(Debug, Clone)]
pub enum Node {
    Identifier(String),
    Integer(u64),
    StringLiteral(Vec<u8>),
    Register(i32),
    PcValue,

    Negation(Box<Self>),
    Plus(Box<Self>, Box<Self>),
    Minus(Box<Self>, Box<Self>),
    Times(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    Shl(Box<Self>, Box<Self>),
    Shr(Box<Self>, Box<Self>),
    Ashr(Box<Self>, Box<Self>),

    Label(String),
    Argument(Box<Node>),
    Instruction(String, Vec<Node>),

    Root(Vec<Node>),
}

impl Node {
    pub fn parse_u64(s: &str, radix: u32) -> Self {
        Node::Integer(u64::from_str_radix(&s.replace("_", ""), radix).unwrap())
    }

    pub fn parse_register(spec: &arch::RiscVSpec, name: &str) -> Result<Self, &'static str> {
        spec.get_register_by_name(name)
            .map_or(Err("invalid register"), |i| Ok(Node::Register(i.index)))
    }

    pub fn simplify(self) -> Self {
        use Node::*;
        match self {
            Negation(box Integer(i)) => Integer(i.wrapping_neg()),
            Plus(box Integer(a), box Integer(b)) => Integer(a.wrapping_add(b)),
            Minus(box Integer(a), box Integer(b)) => Integer(a.wrapping_sub(b)),
            Times(box Integer(a), box Integer(b)) => Integer(a.wrapping_mul(b)),
            Divide(box Integer(a), box Integer(b)) => Integer(a.wrapping_div(b)),
            Shl(box Integer(a), box Integer(b)) => Integer(a << b),
            Shr(box Integer(a), box Integer(b)) => Integer(a >> b),
            Ashr(box Integer(a), box Integer(b)) => Integer((a as i64 >> b as i64) as u64),
            _ => self,
        }
    }
}

pub fn ast_from_str(s: &str, spec: &arch::RiscVSpec) -> Result<Node, grammar::ParseError> {
    grammar::top_level(s, spec)
}

pub fn ast_from_file(path: &str, spec: &arch::RiscVSpec) -> Result<Node, grammar::ParseError> {
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::BufReader;
    let f = File::open(path).unwrap_or_else(|_| panic!("Could not open source file {}", path));
    let mut rd = BufReader::new(f);
    let mut buf = String::new();
    rd.read_to_string(&mut buf)
        .unwrap_or_else(|_| panic!("Could not read from source file {}", path));
    ast_from_str(&buf, spec)
}
