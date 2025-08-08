mod with_argv;

use std::ffi::CStr;

use fspy_shared_unix::exec::ExecResolveConfig;
use libc::{c_char, c_int};
use with_argv::with_argv;

use crate::{
    client::{global_client, raw_exec::RawExec},
    macros::intercept,
};

#[cfg(target_os = "macos")]
pub unsafe fn environ() -> *const *const c_char {
    unsafe { *(libc::_NSGetEnviron().cast()) }
}

#[cfg(target_os = "linux")]
pub unsafe fn environ() -> *const *const c_char {
    unsafe extern "C" {
        static environ: *const *const c_char;
    }
    unsafe { environ }
}

fn handle_exec(
    config: ExecResolveConfig,
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> libc::c_int {
    let client = global_client().expect("exec unexpectedly called before client initialized in ctor");
    let result = unsafe {
        client.handle_spawn(
            config,
            RawExec { prog, argv, envp },
            |raw_command, pre_exec| {
                if let Some(mut pre_exec) = pre_exec {
                    pre_exec.run()?
                };
                Ok(execve::original()(
                    raw_command.prog,
                    raw_command.argv,
                    raw_command.envp,
                ))
            },
        )
    };
    match result {
        Ok(ret) => ret,
        Err(errno) => {
            errno.set();
            -1
        }
    }
}

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
    handle_exec(ExecResolveConfig::search_path_disabled(), prog, argv, envp)
}

intercept!(execvp(64): unsafe extern "C" fn(
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
) -> c_int);
unsafe extern "C" fn execvp(prog: *const c_char, argv: *const *const c_char) -> c_int {
    let _ = execvp::original; // expect original to be unused
    handle_exec(
        ExecResolveConfig::search_path_enabled(None),
        prog,
        argv,
        unsafe { environ() },
    )
}

intercept!(execl(64): unsafe extern "C" fn(path: *const c_char, arg0: *const c_char, ...) -> c_int);
unsafe extern "C" fn execl(path: *const c_char, arg0: *const c_char, valist: ...) -> c_int {
    let _ = execl::original; // expect original to be unused
    eprintln!("execl");
    unsafe {
        with_argv(valist, arg0, |args, _remaining| {
            handle_exec(
                ExecResolveConfig::search_path_disabled(),
                path,
                args.as_ptr(),
                environ(),
            )
        })
    }
}

intercept!(execvpe(64): unsafe extern "C" fn(
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> libc::c_int);
unsafe extern "C" fn execvpe(
    file: *const c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> c_int {
    let _ = execvpe::original;
    handle_exec(
        ExecResolveConfig::search_path_enabled(None),
        file,
        argv,
        envp,
    )
}

// unsafe extern "C" fn execveat(
//     dirfd: c_int,
//     pathname: *const libc::c_char,
//     argv: *const *const libc::c_char,
//     envp: *const *const libc::c_char,
//     flags: c_int,
// ) -> libc::c_int {
//     // TODO: implement flags (AT_EMPTY_PATH/AT_SYMLINK_NOFOLLOW) semantics
//     0
// }

// TODO: execveat/fexecve
