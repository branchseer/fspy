use crate::{client::{global_client, raw_cmd::RawCommand}, macros::intercept};

fn handle_exec(
    find_in_path: bool, 
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> libc::c_int {
    let client = unsafe { global_client() };
    let result = unsafe {client.handle_spawn(find_in_path, RawCommand { prog, argv, envp }, |raw_command, pre_spawn| {
        if let Some(mut pre_spawn) = pre_spawn {
            pre_spawn.run()?
        };
        Ok(execve::original()(raw_command.prog, raw_command.argv, raw_command.envp))
    }) };
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
    handle_exec(false, prog, argv, envp)
}

// TODO: execveat/fexecve/functions in exec(3)
