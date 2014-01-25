# Semi-complete .h generator

Reads in a crate and creates a header out of the exported structs and
`extern "C"` function definitions in it. It currently only supports
primitive (non-vector or string) types in struct fields and function
definitions, and makes no effort to emit enums.

This is highly unlikely to see further work in the near future, and is
meant as somewhat of an example of interfacing with the `syntax` and
`rustc` libraries, although
[#11792](https://github.com/mozilla/rust/issues/11792) means it's not
so great (see `#[no_std]` hack below).

## Example

To run the example in `example`:
- compile `bin.rs` and `example/rust.rs` normally
- uncomment the `#[no_std];` lines & comment the `println!` ones in
  `rust.rs`
- run `header` on `rust.rs`
- compile `c.c` passing the appropriate flags to make it look for the
  Rust crate (`gcc c.c -L. -l$(rustc --crate-file-name rust.rs | sed
  's/^lib\(.*\)so$/\1/')` works for me)
- run the resulting binary to see the exciting Rust <-> C calls (may
  require pointing the dynamic libary loader to the current directory,
  e.g. `LD_LIBRARY_PATH=. ./a.out` on Linux)
