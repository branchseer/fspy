mod caller;
mod client;
mod command;
mod interpose_macros;

use std::{
    ffi::{CStr, OsStr, c_void},
    os::unix::ffi::OsStrExt,
};

use allocator_api2::vec::Vec;
use bstr::BStr;
use bumpalo::Bump;
use caller::caller_dli_fname;
use client::{CLIENT, RawCommand};
use interpose_macros::interpose_libc;
use libc::{c_char, c_int};

use crate::consts::AccessMode;

unsafe extern "C" fn open(path_ptr: *const c_char, flags: c_int, mut args: ...) -> c_int {
    let path = BStr::new(unsafe { CStr::from_ptr(path_ptr) }.to_bytes());
    let caller = BStr::new(caller_dli_fname!().unwrap_or(b""));
    // CLIENT.send(AccessMode::Read, path, caller);
    // https://github.com/rust-lang/rust/issues/44930
    // https://github.com/thepowersgang/va_list-rs/
    // https://github.com/mstange/samply/blob/02a7b3771d038fc5c9226fd0a6842225c59f20c1/samply-mac-preload/src/lib.rs#L85-L93
    // https://github.com/apple-oss-distributions/xnu/blob/e3723e1f17661b24996789d8afc084c0c3303b26/libsyscall/wrappers/open-base.c#L85
    if flags & libc::O_CREAT != 0 {
        // https://github.com/tailhook/openat/issues/21#issuecomment-535914957
        let mode: libc::c_int = unsafe { args.arg() };
        unsafe { libc::open(path_ptr, flags, mode) }
    } else {
        unsafe { libc::open(path_ptr, flags) }
    }
}
// interpose_libc!(open);

unsafe extern "C" fn execve(
    prog: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> libc::c_int {
    let bump = Bump::new();
    let mut raw_cmd = RawCommand {
        prog: prog.cast(),
        argv: argv.cast(),
        envp: envp.cast(),
    };
    if let Err(err) = unsafe { CLIENT.interpose_command(&bump, &mut raw_cmd) } {
        err.set();
        return -1;
    }
    unsafe {
        libc::execve(
            raw_cmd.prog.cast(),
            raw_cmd.argv.cast(),
            raw_cmd.envp.cast(),
        )
    }
}
interpose_libc!(execve);

unsafe extern "C" fn posix_spawn(
    pid: *mut libc::pid_t,
    path: *const c_char,
    file_actions: *const libc::posix_spawn_file_actions_t,
    attrp: *const libc::posix_spawnattr_t,
    argv: *const *mut c_char,
    envp: *const *mut c_char,
) -> libc::c_int {
    let bump = Bump::new();
    let mut raw_cmd = RawCommand {
        prog: path,
        argv: argv.cast(),
        envp: envp.cast(),
    };

    if let Err(err) = unsafe { CLIENT.interpose_command(&bump, &mut raw_cmd) } {
        return err as c_int;
    }
    unsafe {
        libc::posix_spawn(
            pid,
            raw_cmd.prog,
            file_actions,
            attrp,
            raw_cmd.argv.cast(),
            raw_cmd.envp.cast(),
        )
    }
}
interpose_libc!(posix_spawn);

unsafe extern "C" fn posix_spawnp(
    pid: *mut libc::pid_t,
    file: *const c_char,
    file_actions: *const libc::posix_spawn_file_actions_t,
    attrp: *const libc::posix_spawnattr_t,
    argv: *const *mut c_char,
    envp: *const *mut c_char,
) -> libc::c_int {
    let bump = Bump::new();
    let file = OsStr::from_bytes(unsafe { CStr::from_ptr(file.cast()) }.to_bytes());
    let Ok(file) = which::which(file) else {
        return nix::Error::ENOENT as c_int;
    };
    let file = RawCommand::to_c_str(&bump, file.as_os_str());

    let mut raw_cmd = RawCommand {
        prog: file,
        argv: argv.cast(),
        envp: envp.cast(),
    };
    if let Err(err) = unsafe { CLIENT.interpose_command(&bump, &mut raw_cmd) } {
        return err as c_int;
    }
    unsafe {
        libc::posix_spawnp(
            pid,
            raw_cmd.prog,
            file_actions,
            attrp,
            raw_cmd.argv.cast(),
            raw_cmd.envp.cast(),
        )
    }
}

interpose_libc!(posix_spawnp);
