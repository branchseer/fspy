use std::{os::unix::net::UnixDatagram, process::Command};


fn main() {
    // let (x, y) = UnixDatagram::pair().unwrap();
    // dbg!(Command::new("echo").status());
    // drop(y);
    // let mut buf = [0u8; 24];
    // dbg!(x.recv(&mut buf));
    fspy::debug_example();
}
