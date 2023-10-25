use crate::arch;
use crate::parser::Node;

peg::parser! { grammar asmpeg(spec: &arch::RiscVSpec) for str {
rule comment() = quiet!{";" (!['\n'][_])+}
rule whitechar() = quiet!{[' '|'\r'|'\t']} / quiet!{comment()} / "\\\n"
rule whitespace() = quiet!{whitechar()+}
rule newline() = quiet!{whitespace()?} "\n"
rule register() -> Node = quiet!{ s:$(['a'..='z'|'A'..='Z'|'.'|'_']['a'..='z'|'A'..='Z'|'0'..='9'|'.'|'_']*) {? Node::parse_register(spec, s) } } / expected!("register")
rule idstr() -> &'input str = quiet!{ !register() sv:$(['a'..='z'|'A'..='Z'|'.'|'_']['a'..='z'|'A'..='Z'|'0'..='9'|'.'|'_']*) { sv } } / expected!("identifier")
rule identifier() -> Node = s:idstr() { Node::Identifier(s.to_owned()) }

rule integer() -> Node = quiet!{ "0x" n:$(['0'..='9'|'a'..='f'|'A'..='F'|'_']+) { Node::parse_u64(n, 16) } }
        / quiet!{ "0o" n:$(['0'..='7'|'_']+) { Node::parse_u64(n, 8) } }
        / quiet!{ "0b" n:$(['0'..='1'|'_']+) { Node::parse_u64(n, 2) } }
        / quiet!{ "0d"? n:$(['0'..='9'|'_']+) { Node::parse_u64(n, 10) } }
        / expected!("integer")

rule escape() -> u8 = _:"\\n" {"\n".as_bytes()[0]} / _:"\\t" {"\t".as_bytes()[0]}
       / _:"\\\\" {"\\".as_bytes()[0]} / _:"\\r" {"\r".as_bytes()[0]}
       / "\\x" n:$(['0'..='9'|'a'..='f'|'A'..='F'|'_']*<2>) { u64::from_str_radix(n, 16).unwrap() as u8 }

rule str_char<Q>(quote: rule<Q>) -> u8 = escape() / c:$(!quote() [_]) { c.as_bytes()[0] }

rule char_literal() -> Node = "'" s:str_char(<"'">) "'" { Node::Integer(s as u64) }
rule bytes_literal() -> Node = "\"" s:str_char(<"\"">)* "\"" { Node::StringLiteral(s) }

rule negation() -> Node = "-" e:expression() { Node::Negation(Box::new(e)) }
pub rule expr_atom() -> Node = whitespace()? "(" whitespace()? e:expression() whitespace()? ")" whitespace()? {e.simplify()}
                      / whitespace()? n:negation() whitespace()? {n.simplify()}
                      / whitespace()? i:integer() whitespace()? {i}
                      / whitespace()? i:identifier() whitespace()? {i}
                      / whitespace()? "$" whitespace()? { Node::PcValue }
                      / whitespace()? c:char_literal() whitespace()? {c}

pub rule expression() -> Node = precedence! {
      x:(@) "<<" y:@ { Node::Shl(Box::new(x), Box::new(y)).simplify() }
      x:(@) ">>" y:@ { Node::Shr(Box::new(x), Box::new(y)).simplify() }
      x:(@) ">>>" y:@ { Node::Ashr(Box::new(x), Box::new(y)).simplify() }
      --
       x:(@) "+" y:@ { Node::Plus(Box::new(x), Box::new(y)).simplify() }
      x:(@) "-" y:@ { Node::Minus(Box::new(x), Box::new(y)).simplify() }
      --
       x:(@) "*" y:@ { Node::Times(Box::new(x), Box::new(y)).simplify() }
      x:(@) "/" y:@ { Node::Divide(Box::new(x), Box::new(y)).simplify() }
      --
      a:expr_atom() {a}
}

pub rule label() -> Node = whitespace()? i:idstr() whitespace()? ":" { Node::Label(i.to_owned()) } / expected!("label")
pub rule argument() -> Node = whitespace()? e:(register() / expression()) whitespace()? {Node::Argument(Box::new(e))}
rule instruction0() -> Node = whitespace()? nm:idstr() whitespace()? { Node::Instruction(nm.to_owned(), vec![]) }
rule instruction1() -> Node = whitespace()? nm:idstr() whitespace() a0:argument() whitespace()? { Node::Instruction(nm.to_owned(), vec![a0]) }
rule instructionN() -> Node = whitespace()? nm:idstr() whitespace() a0:argument() aN:( "," an:argument() {an} )+ {
    let mut v = aN;
    v.insert(0, a0);
    Node::Instruction(nm.to_owned(), v)
}
pub rule instruction() -> Node = instructionN() / instruction1() / instruction0() / expected!("instruction")

pub rule top_element() -> Node = (whitespace() / newline())* n:(label() / instruction()) {n}
pub rule top_level() -> Node = n:(top_element()*) (whitespace() / newline())* { Node::Root(n) }

}}

//include!{"../expanded.rs"}

pub use asmpeg::top_level;
