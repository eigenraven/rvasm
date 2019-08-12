#![feature(box_syntax)]
#![feature(box_patterns)]
#![warn(clippy::all)]
#![allow(dead_code)]
mod arch;
mod emit;
mod parser;
mod test;

use emit::flatbin;
use std::io::prelude::*;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, Copy, Clone, StructOpt)]
enum OutputFormat {
    Flat,
}
impl std::str::FromStr for OutputFormat {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_ref() {
            "flat" => Ok(OutputFormat::Flat),
            _ => Err("Invalid output format specified"),
        }
    }
}

#[derive(Debug, Clone, StructOpt)]
#[structopt(
    name = "rvasm",
    about = "Usage of the rvasm RISC-V assembler",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Opt {
    #[structopt(help = "Input file path")]
    input_file: Option<PathBuf>,

    #[structopt(
        short = "s",
        long = "string",
        help = "Input string instead of file, all semicolons are replaced by newlines"
    )]
    input_string: Option<String>,

    #[structopt(
        short = "o",
        long = "output-file",
        help = "Output (assembled) file path"
    )]
    output_file: PathBuf,

    #[structopt(short = "v", long = "verbose", help = "Enable additional output")]
    verbose: bool,

    #[structopt(
        short = "f",
        long = "format",
        default_value = "flat",
        help = "Output file format (only `flat` binary is supported)"
    )]
    output_format: OutputFormat,

    #[structopt(
        short = "c",
        long = "cfg",
        help = "Additional config file paths to parse"
    )]
    cfg: Vec<PathBuf>,

    #[structopt(
        short = "a",
        long = "arch",
        default_value = "RV32I",
        help = "RISC-V variant to assemble for, like RV32IMZamZifencei (finds config files in standard path)"
    )]
    arch: String,

    #[structopt(
        short = "b",
        long = "binary",
        help = "In addition to writing a file, print the assembly in binary to the terminal"
    )]
    print_binary: bool,
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

    let mut std_path = Vec::new();
    std_path.push(PathBuf::from("./cfg/"));

    let mut rv = crate::arch::RiscVSpec::new();
    if let Err(e) = rv.load_arch_cfg(&std_path, &opt.arch, opt.verbose) {
        eprintln!("Error loading arch-defined configuration: {:?}", e);
        std::process::exit(1);
    }
    for cfg in opt.cfg {
        if let Err(e) = rv.load_single_cfg_file(&cfg) {
            let pstr = cfg.as_os_str().to_string_lossy();
            eprintln!(
                "Error loading additional configuration from {}: {:?}",
                pstr, e
            );
            std::process::exit(1);
        }
    }

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
        ast = parser::ast_from_str(&istr.replace(";", "\n"), &rv);
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
    if let Err(e) = ast {
        eprintln!("Parse error: {:?}", e);
        std::process::exit(1);
    }
    let ast = ast.unwrap();

    use std::convert::TryInto;
    let bin: Vec<u8>;

    match opt.output_format {
        OutputFormat::Flat => {
            let ebin = flatbin::emit_flat_binary(&rv, &ast);
            if let Err(e) = ebin {
                eprintln!("Binary emission error: {:?}", e);
                std::process::exit(1);
            } else {
                bin = ebin.unwrap();
            }
        }
    }

    if opt.print_binary {
        println!("Binary assembly:");
        let mut cnt = 0;
        for word in bin.chunks(4) {
            let word: [u8; 4] = word.try_into().unwrap();
            print!("{:032b} ", u32::from_le_bytes(word));
            if cnt == 1 {
                println!();
                cnt = 0;
            } else {
                cnt += 1;
            }
        }
        println!();
    }

    std::fs::File::create(opt.output_file)
        .expect("Could not open output file for writing")
        .write_all(&bin)
        .expect("Could not write to output file");
}
