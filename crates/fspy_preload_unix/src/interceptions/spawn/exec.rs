use crate::{interceptions::spawn::exec::execve::original, macros::intercept};

intercept!(execve(64): unsafe extern "C" fn(
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> libc::c_int);
unsafe extern "C" fn execve(
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> libc::c_int {
    eprintln!("execve");
    unsafe { original()(prog, argv, envp) }
}

// TODO: execveat/fexecve