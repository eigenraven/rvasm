
#[test]
fn test_parser_on_all_inputs() {
    use std::path::Path;
    use crate::parser::ast_from_file;

    let mut rv32i_str = String::new();
    use std::io::prelude::*;
    std::fs::File::open("./cfg/rv32i.toml")
        .unwrap()
        .read_to_string(&mut rv32i_str)
        .unwrap();
    let mut rv = crate::arch::RiscVSpec::new();
    rv.load_single_cfg_string(&rv32i_str).expect("Parse error");

    let dir = Path::new("./test/").read_dir().expect("Can't open ./test folder of sample inputs");
    for entry in dir {
        if entry.is_err() {
            continue;
        }
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_file() {
            continue;
        }
        let epath = entry.path();
        let path = epath.to_str().unwrap();
        if path.ends_with(".s") {
            eprint!(" * parsing {} ...", path);
            ast_from_file(path, &rv).expect("Testcase parsing failed");
            eprintln!("ok");
        }
    }
}
