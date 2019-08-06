
#[test]
fn test_parser_on_all_inputs() {
    use std::path::Path;
    use crate::parser::Ast;
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
            Ast::from_file(path);
            eprintln!("ok");
        }
    }
}
