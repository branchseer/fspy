use std::{env::args_os, io::{stdout, Write}, os::unix::ffi::OsStrExt};

fn main() {
    // let argv = args_os();
    let mut out = stdout().lock();
    for arg in args_os() {
        out.write_all(arg.as_bytes()).unwrap();
        out.write_all(b"$\n").unwrap();
    }
}
