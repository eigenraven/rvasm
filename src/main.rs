mod arch;
//mod parser;
//mod test;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "rvasm", about = "Usage of the rvasm RISC-V assembler")]
struct Opt {
    input_file: Option<PathBuf>,
    #[structopt(short = "s", long = "string")]
    input_string: Option<String>
}

fn main() {
    let opt = Opt::from_args();
    if opt.input_string.is_none() && opt.input_file.is_none() {
        Opt::clap().print_long_help().unwrap();
        eprintln!("A source file or string is required");
        return;
    }
    if opt.input_string.is_some() && opt.input_file.is_some() {
        Opt::clap().print_long_help().unwrap();
        eprintln!("Only one source allowed: either a file or a string");
        return;
    }
    println!("{:?}", opt);
    /*let ast;
    if let Some(ref istr) = opt.input_string {
        ast = parser::Ast::from_str(istr, "./CMDLINE");
    } else {
        ast = parser::Ast::from_file(opt.input_file.as_ref().unwrap().to_str().expect("Invalid Unicode in specified file path"));
    }
    println!("{:?}", ast);*/
}
