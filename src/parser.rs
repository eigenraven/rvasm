use crate::arch;

#[derive(Clone, Debug)]
pub enum InsnParam {
    Register(arch::Register),
    Immediate(i32),
    Symbol(SourceSlice),
}

#[derive(Copy, Clone, Debug)]
pub struct SourceSlice {
    pub source_idx: usize,
    pub start: usize,
    pub end: usize,
}

impl SourceSlice {
    pub fn get<'a>(&self, ast: &'a Ast) -> &'a str {
        &ast.source_set[self.source_idx][self.start..self.end]
    }

    pub fn new(source_idx: usize, start: usize, end: usize) -> Self {
        SourceSlice {
            source_idx,
            start,
            end,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NodeType {
    RootNode,
    Instruction {name: SourceSlice},
    InstructionArgument (InsnParam)
}

#[derive(Copy, Clone, Debug)]
pub struct SourceLocation {
    /// Index into the Ast's source_set
    pub path: SourceSlice,
    pub line: usize,
    pub col: usize,
}

impl SourceLocation {
    pub fn new(path: SourceSlice, line: usize, col: usize) -> Self {
        Self { path, line, col }
    }
}

pub type NodeIndex = usize;

#[derive(Clone, Debug)]
pub struct Node {
    pub ntype: NodeType,
    pub loc: SourceLocation,
    /// Vector of indices into nodes
    pub children: Vec<NodeIndex>,
}

#[derive(Clone, Debug, Default)]
pub struct Ast {
    pub source_set: Vec<String>,
    /// The set of owned nodes, the root node is the first node
    pub nodes: Vec<Node>,
}

const fn ascii_const(c: char) -> u8 {
    (c as u32) as u8
}

impl Ast {
    pub fn new() -> Self {
        Ast::default()
    }

    pub fn from_str(src: &str, path: &str) -> Self {
        let mut ast = Self::new();
        ast.parse_str(src, path);
        ast
    }

    pub fn from_file(path: &str) -> Self {
        let mut ast = Self::new();
        ast.parse_file(path);
        ast
    }

    pub fn parse_str(&mut self, src: &str, path: &str) -> NodeIndex {
        let path_slice = SourceSlice::new(self.source_set.len(), 0, path.len());
        self.source_set.push(path.to_owned());
        let src_slice = SourceSlice::new(self.source_set.len(), 0, src.len());
        self.source_set.push(src.to_owned());
        let root_node = Node {
            ntype: NodeType::RootNode,
            loc: SourceLocation::new(path_slice, 0, 0),
            children: Vec::new(),
        };
        let node_idx = self.nodes.len();
        self.nodes.push(root_node);

        self.do_parse(node_idx, src_slice, path_slice);
        node_idx
    }

    pub fn parse_file(&mut self, path: &str) -> NodeIndex {
        use std::fs::File;
        use std::io::prelude::*;
        use std::io::BufReader;
        let f = File::open(path).expect(&format!("Could not open source file {}", path));
        let mut rd = BufReader::new(f);
        let mut buf = String::new();
        rd.read_to_string(&mut buf)
            .expect(&format!("Could not read from source file {}", path));
        self.parse_str(&buf, path)
    }

    fn gen_node(&self, ntype: NodeType, loc: SourceLocation) -> (NodeIndex, Node) {
        let idx = self.nodes.len();
        let node = Node {
            ntype,
            loc,
            children: Vec::new(),
        };
        (idx, node)
    }

    fn do_parse(&mut self, root_node: NodeIndex, src: SourceSlice, path: SourceSlice) {
        let src_str = self.source_set[src.source_idx].as_bytes();
        let mut lineno = 1;
        let mut linestart = 0;
        let mut idx = src.start;

        // returns a token and true if it's not the end of the line
        // idx should point at first char of token or whitespace/comma before
        let next_token = |idx: &mut usize| -> (Option<SourceSlice>, bool) {
            // skip initial whitespace (not newlines)
            while *idx < src.end && (char::from(src_str[*idx]).is_whitespace() || src_str[*idx] == ascii_const(',')) && src_str[*idx] != ascii_const('\n') {
                *idx += 1;
            }
            let startidx = *idx;
            let mut hasnext = true;
            while *idx < src.end && !char::from(src_str[*idx]).is_whitespace() && src_str[*idx] != ascii_const(',') && src_str[*idx] != ascii_const(';') {
                *idx += 1;
            }
            if *idx >= src.end || src_str[*idx] == ascii_const('\n') || src_str[*idx] == ascii_const('\r') || src_str[*idx] == ascii_const(';') {
                hasnext = false;
            }
            if *idx == startidx {
                (None, hasnext)
            } else {
                (Some(SourceSlice::new(src.source_idx, startidx, *idx)), hasnext)
            }
        };

        let parse_iarg = |t: SourceSlice, s: &str| -> Result<InsnParam, ()> {
            if let Some(reg) = arch::Register::from_name(s) {
                Ok(InsnParam::Register(reg))
            } else if let Ok(val) = i32::from_str_radix(s, 10) {
                Ok(InsnParam::Immediate(val))
            } else {
                Err(())
            }
        };

        while idx < src.end {
            let startidx = idx;
            let ch = char::from(src_str[idx]);
            let nx_ch = if idx + 1 >= src.end {
                '\0'
            } else {
                char::from(src_str[idx + 1])
            };
            idx += 1;
            // Newlines
            if ch == '\r' {
                continue;
            } else if ch == '\n' {
                lineno += 1;
                linestart = idx;
                continue;
            }
            // General whitespace -> ignore
            else if ch.is_whitespace() {
                continue;
            }
            // Comment
            else if ch == ';' {
                while idx < src.end && src_str[idx] != ascii_const('\n') {
                    idx += 1;
                }
                continue;
            }
            // Directives
            // Instruction
            else if ch.is_ascii_alphabetic() {
                idx -= 1;
                let (iname, mut hasargs) = next_token(&mut idx);
                idx += 1;
                let iname = iname.unwrap();
                let inode = {
                    let (i,n) = self.gen_node(NodeType::Instruction{name:iname}, SourceLocation::new(path, lineno, startidx - linestart));
                    self.nodes.push(n);
                    self.nodes[root_node].children.push(i);
                    i};
                while hasargs {
                    let (tk, hasnext) = next_token(&mut idx);
                    hasargs = hasnext && tk.is_some();
                    if let Some(t) = tk {
                        let iarg: InsnParam = parse_iarg(t, t.get(&self)).expect(&format!("Can't parse instruction argument `{:?}` at {}:{} in file {}", t.get(&self), lineno, startidx - linestart, path.get(&self)));
                        let (i,n) = self.gen_node(NodeType::InstructionArgument(iarg), SourceLocation::new(path, lineno, t.start - linestart));
                        self.nodes.push(n);
                        self.nodes[inode].children.push(i);
                    }
                }
                continue;
            } else {
                panic!("Unknown character `{:?}` at {}:{} in file {}", ch, lineno, startidx - linestart, path.get(&self));
            }
        }
    }
}
