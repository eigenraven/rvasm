# A RISC-V assembler

An extensible RISC-V assembler written in Rust.
The goal of this project is to provide a platform for experimentation with the RISC-V architecture by having
an assembler that allows for quick iteration of tiny programs compiled into flat binaries, and of instruction
sets defined in simple to edit [TOML 0.5](https://github.com/toml-lang/toml) files.

## Usage
```
USAGE:
    rvasm [FLAGS] [OPTIONS] [--] [input_file]

FLAGS:
    -h, --help       Prints help information
    -b, --binary     In addition to writing a file, print the assembly in binary to the terminal
    -V, --version    Prints version information
    -v, --verbose    Enable additional output

OPTIONS:
    -a, --arch <arch>                  RISC-V variant to assemble for, like RV32IMZamZifencei (finds config files in
                                       standard path) [default: RV32I]
    -c, --cfg <cfg>...                 Additional config file paths to parse
    -s, --string <input_string>        Input string instead of file, all semicolons are replaced by newlines
    -o, --output-file <output_file>    Output (assembled) file path
    -f, --format <output_format>       Output file format (only `flat` binary is supported) [default: flat]

ARGS:
    <input_file>    Input file path
```

For example, if you have a file `sample1.s`:
```
addi s0, s1, 2+2
```
You can assemble it by using the command `rvasm sample1.s -o sample1.bin`.
This is the equivalent of options: `rvasm sample1.s -o sample1.bin -a RV32I -f flat`

If you'd like to peek into the binary representation of instructions (only 32-bit ILEN supported now),
you can invoke rvasm like this:
```
rvasm -s "addi s0, s1, 2+2" -b
```
Which will produce the following output:
```
Binary assembly:
00000000010001001000010000010011 
Warning: no output file specified so none was created.
```
Which displays the 32-bit instructions as 32 binary digits, rightmost one is the LSB and leftmost is MSB.
(Swapped around from the actual little endian byte encoding for readability)

## Defining instruction sets
Create a copy of [cfg/help.toml](cfg/help.toml) and follow the comments to define instruction formats and specific encodings.
You can also take a look at the included RV32I definition in [cfg/rv32i.toml](cfg/rv32i.toml).

## Supported directives
Apart from the instructions defined in the TOML files, the assembler supports a few directives:

* `$` - replaced by current PC value
* `.org ADDRESS` - sets the internal PC value and output file position to `ADDRESS`
* `.equ NAME VAL`/`.define NAME VAL` - defines constants that can be used in expressions instead of integers
* `.label:` - labels starting with a dot are local to the scope of their parent label
