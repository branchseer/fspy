use std::{env::current_dir, ptr::null};

fn main() {
    // dbg!(current_dir());
    let prog = c"x.sh".as_ptr();
    let argv = &[prog, null()];
    let envp = &[null()];
    unsafe { libc::execve(prog, argv.as_ptr(), envp.as_ptr()) };
    dbg!(unsafe { *libc::__errno_location() });
}
