#[crate_id="rust_example"];
#[crate_type="dylib"];
#[no_std]; // FIXME #11792... required to actually generate the header :(

#[no_mangle]
pub extern "C" fn hi(x: int) -> ~bool {
    //println!("hello {}", x); // comment out for .h generation
    ~true
}
#[no_mangle]
pub extern "C" fn bye(x: &bool) {
    if *x {
        //println!("bye"); // comment out for .h generation
    }
}
