use crate::arch;
use crate::parser::Node;
use smallvec::SmallVec;

#[derive(Clone, Debug)]
pub enum EmitError {
    UnexpectedNodeType(String),
    InvalidInstruction(String),
    InvalidArgumentCount(String),
    InvalidArgumentType(String, usize),
    InvalidEncoding(String),
}

pub fn emit_flat_binary(spec: &arch::RiscVSpec, ast: &Node) -> Result<Vec<u8>, EmitError> {
    let mut state = BinaryEmitState {
        out_buf: Vec::new(),
        out_pos: 0,
    };
    emit_binary_recurse(spec, &mut state, ast).map(move |_| state.out_buf)
}

#[derive(Debug)]
struct BinaryEmitState {
    out_buf: Vec<u8>,
    out_pos: usize,
}

impl BinaryEmitState {
    fn accomodate_bytes(&mut self, byte_count: usize) -> &mut [u8] {
        let start_pos = self.out_pos;
        let end_pos = start_pos + byte_count;
        if self.out_buf.len() < end_pos {
            self.out_buf.resize(end_pos, 0);
        }
        self.out_pos = end_pos;
        &mut self.out_buf[start_pos..end_pos]
    }
}

fn emit_binary_recurse(
    spec: &arch::RiscVSpec,
    state: &mut BinaryEmitState,
    node: &Node,
) -> Result<(), EmitError> {
    use Node::*;

    let ialign_bytes = (spec.get_const("IALIGN").unwrap_or(32) as usize + 7) / 8;
    let max_ilen_bytes = (spec.get_const("ILEN").unwrap_or(32) as usize + 7) / 8;

    match node {
        Root(nodes) => {
            for node in nodes.iter() {
                emit_binary_recurse(spec, state, node)?;
            }
            Ok(())
        }
        Label(lname) => Ok(()),
        Instruction(iname, args) => {
            match iname.as_ref() {
                // .org ADDRESS
                ".org" | ".ORG" => {
                    if args.len() != 1 {
                        return Err(EmitError::InvalidArgumentCount(iname.clone()));
                    }
                    if let Node::Integer(adr) = args[0] {
                        let new_out_pos = adr as usize;
                        if new_out_pos > state.out_buf.len() {
                            state
                                .out_buf
                                .reserve(new_out_pos - state.out_buf.len() + 32 * 32);
                            state.out_buf.resize(new_out_pos, 0);
                        }
                        state.out_pos = new_out_pos;
                        Ok(())
                    } else {
                        Err(EmitError::InvalidArgumentType(iname.clone(), 0))
                    }
                }
                // Standard RISC-V instructions
                _ => {
                    // check spec
                    let specinsn = spec
                        .get_instruction_by_name(iname)
                        .ok_or_else(|| EmitError::InvalidInstruction(iname.clone()))?;
                    let fmt = specinsn.get_format(&spec);
                    if args.len() != specinsn.args.len() {
                        return Err(EmitError::InvalidArgumentCount(iname.clone()));
                    }
                    let mut argv: SmallVec<[u64; 4]> = SmallVec::new();
                    for (i, arg) in args.iter().enumerate() {
                        match fmt.fields[specinsn.args[i]].vtype {
                            arch::FieldType::Value => {
                                if let Node::Argument(box Node::Integer(val)) = arg {
                                    argv.push(*val);
                                } else {
                                    return Err(EmitError::InvalidArgumentType(iname.clone(), i));
                                }
                            }
                            arch::FieldType::Register => {
                                if let Node::Argument(box Node::Register(rid)) = arg {
                                    argv.push(*rid as u64);
                                } else {
                                    return Err(EmitError::InvalidArgumentType(iname.clone(), i));
                                }
                            }
                        }
                    }
                    assert_eq!(argv.len(), specinsn.args.len());
                    // check length
                    let ilen_bytes = (fmt.ilen + 7) / 8;
                    if ilen_bytes > max_ilen_bytes {
                        return Err(EmitError::InvalidEncoding(iname.clone()));
                    }
                    // check alignment
                    let aligned_pos =
                        (state.out_pos + ialign_bytes - 1) / ialign_bytes * ialign_bytes;
                    if state.out_pos != aligned_pos {
                        // pad out with zeroes
                        // TODO: NOP alignment instead of zero alignment
                        state.accomodate_bytes(aligned_pos - state.out_pos);
                    }
                    // emit instruction
                    let bytes = state.accomodate_bytes(ilen_bytes);
                    specinsn
                        .encode_into(bytes, spec, argv.as_slice())
                        .map_err(|_| EmitError::InvalidEncoding(iname.clone()))
                }
            }
        }
        _ => Err(EmitError::UnexpectedNodeType(format!("{:?}", node))),
    }
}
