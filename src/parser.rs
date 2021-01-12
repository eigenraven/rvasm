use crate::arch;
use crate::grammar;

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

    /// Returns: the simplified node and whether all the constants were reduced to integers.
    pub fn emitter_simplify<F: Fn(&str) -> Option<u64>>(
        &self,
        const_provider: &F,
        pc: u64,
    ) -> (Self, bool) {
        use Node::*;
        let cloned_f = || (self.clone(), false);
        let cloned_t = || (self.clone(), true);
        match self {
            Identifier(ident) => const_provider(ident)
                .map(|v| (Integer(v), true))
                .unwrap_or_else(cloned_f),
            Label(lname) => const_provider(lname)
                .map(|v| (Integer(v), true))
                .unwrap_or_else(cloned_f),

            Integer(v) => (Integer(*v), true),
            StringLiteral(_) => cloned_t(),
            Register(_) => cloned_t(),
            PcValue => (Integer(pc), true),

            Negation(box a) => {
                let sa = a.emitter_simplify(const_provider, pc);
                (Negation(box sa.0).simplify(), sa.1)
            }
            Plus(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Plus(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }
            Minus(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Minus(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }
            Times(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Times(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }
            Divide(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Divide(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }
            Shl(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Shl(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }
            Shr(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Shr(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }
            Ashr(box a, box b) => {
                let sa = a.emitter_simplify(const_provider, pc);
                let sb = b.emitter_simplify(const_provider, pc);
                (Ashr(box sa.0, box sb.0).simplify(), sa.1 && sb.1)
            }

            Argument(box node) => {
                let s = node.emitter_simplify(const_provider, pc);
                (Argument(box s.0), s.1)
            }
            Instruction(iname, args) => {
                let mut succ = true;
                let mut sargs = Vec::new();
                for arg in args.iter() {
                    let s = arg.emitter_simplify(const_provider, pc);
                    sargs.push(s.0);
                    succ &= s.1;
                }
                (Instruction(iname.to_owned(), sargs), succ)
            }

            Root(nodes) => {
                let mut succ = true;
                let mut snodes = Vec::new();
                for node in nodes.iter() {
                    let s = node.emitter_simplify(const_provider, pc);
                    snodes.push(s.0);
                    succ &= s.1;
                }
                (Root(snodes), succ)
            }
        }
    }
}

pub type ParseError = peg::error::ParseError<peg::str::LineCol>;

pub fn ast_from_str(s: &str, spec: &arch::RiscVSpec) -> Result<Node, ParseError> {
    grammar::top_level(s, spec)
}

pub fn ast_from_file(path: &str, spec: &arch::RiscVSpec) -> Result<Node, ParseError> {
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
