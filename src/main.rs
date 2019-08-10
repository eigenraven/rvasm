#![feature(box_syntax)]
#![warn(clippy::all)]
#![allow(dead_code)]
mod arch;
mod parser;
mod test;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
#[structopt(name = "rvasm", about = "Usage of the rvasm RISC-V assembler")]
struct Opt {
    input_file: Option<PathBuf>,
    #[structopt(short = "s", long = "string")]
    input_string: Option<String>,

    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
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

    let mut rv32i_str = String::new();
    use std::io::prelude::*;
    std::fs::File::open("./cfg/rv32i.toml")
        .unwrap()
        .read_to_string(&mut rv32i_str)
        .unwrap();
    let mut rv = crate::arch::RiscVSpec::new();
    rv.load_single_cfg_string(&rv32i_str).expect("Parse error");

    if opt.verbose {
        for abi in rv.get_loaded_abis() {
            println!(
                "Loaded ABI: {} - '{}' based on spec '{}'",
                abi.code, abi.name, abi.spec
            );
        }
    }

    let ast;
    if let Some(ref istr) = opt.input_string {
        ast = parser::ast_from_str(istr, &rv);
    } else {
        ast = parser::ast_from_file(
            opt.input_file
                .as_ref()
                .unwrap()
                .to_str()
                .expect("Invalid Unicode in specified file path"),
            &rv,
        );
    }
    println!("{:?}", ast);
}
