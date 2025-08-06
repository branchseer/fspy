use std::thread;

use libc::{c_char, c_int};

use crate::{
    client::{global_client, raw_exec::RawExec},
    macros::intercept,
};

type PosixSpawnFn = unsafe extern "C" fn(
    pid: *mut libc::pid_t,
    prog: *const c_char,
    file_actions: *const libc::posix_spawn_file_actions_t,
    attrp: *const libc::posix_spawnattr_t,
    argv: *const *mut c_char,
    envp: *const *mut c_char,
) -> libc::c_int;

unsafe fn handle_posix_spawn(
    find_in_path: bool,
    original: PosixSpawnFn,
    pid: *mut libc::pid_t,
    file: *const c_char,
    file_actions: *const libc::posix_spawn_file_actions_t,
    attrp: *const libc::posix_spawnattr_t,
    argv: *const *mut c_char,
    envp: *const *mut c_char,
) -> c_int {
    struct AssertSend<T>(T);
    unsafe impl<T> Send for AssertSend<T> {}

    let client = global_client();
    let result = unsafe {
        client.handle_spawn::<c_int>(
            find_in_path,
            RawExec {
                prog: file,
                argv: argv.cast(),
                envp: envp.cast(),
            },
            |raw_command, pre_spawn| {
                let call_original = move || {
                    original(
                        pid,
                        raw_command.prog,
                        file_actions,
                        attrp,
                        raw_command.argv.cast(),
                        raw_command.envp.cast(),
                    )
                };
                if let Some(mut pre_spawn) = pre_spawn {
                    thread::scope(move |s| {
                        let call_original = AssertSend(call_original);
                        s.spawn(move || {
                            let call_original = call_original;
                            pre_spawn.run()?;

                            nix::Result::Ok((call_original.0)())
                        })
                        .join()
                        .unwrap()
                    })
                } else {
                    Ok(call_original())
                }
            },
        )
    };
    match result {
        Err(errno) => errno as _,
        Ok(ret) => ret,
    }
}


intercept!(posix_spawnp(64): PosixSpawnFn);
unsafe extern "C" fn posix_spawnp(
    pid: *mut libc::pid_t,
    file: *const c_char,
    file_actions: *const libc::posix_spawn_file_actions_t,
    attrp: *const libc::posix_spawnattr_t,
    argv: *const *mut c_char,
    envp: *const *mut c_char,
) -> libc::c_int {
   unsafe { handle_posix_spawn(true, posix_spawnp::original(), pid, file, file_actions, attrp, argv, envp) }
}

intercept!(posix_spawn(64): PosixSpawnFn);
unsafe extern "C" fn posix_spawn(
    pid: *mut libc::pid_t,
    file: *const c_char,
    file_actions: *const libc::posix_spawn_file_actions_t,
    attrp: *const libc::posix_spawnattr_t,
    argv: *const *mut c_char,
    envp: *const *mut c_char,
) -> libc::c_int {
   unsafe { handle_posix_spawn(false, posix_spawn::original(), pid, file, file_actions, attrp, argv, envp) }
}
