use std::{convert::identity, ffi::{CStr, OsStr, VaList}, mem::zeroed, os::{raw::c_void, unix::ffi::OsStrExt}, path::Path, ptr::{null, null_mut}};

use allocator_api2::vec::Vec;
use bumpalo::Bump;
use client::{RawCommand, CLIENT};
use command::Command;
use libc::{c_char, c_int, mode_t};

mod command;
mod client;


#[repr(C)]
struct InterposeEntry {
    _new: *const c_void,
    _old: *const c_void,
}

macro_rules! interpose {
    ($target:expr => unsafe extern "C" fn ($($param_ident:ident : $param_ty:ty),* $(,)?) -> $ret:ty $blk:block) => {
        const _: () = {
            const _: unsafe extern "C" fn($($param_ident: $param_ty),*) -> $ret = $target;
            unsafe extern "C" fn hook_fn($($param_ident: $param_ty),*) -> $ret $blk

            #[used]
            #[allow(dead_code)]
            #[allow(non_upper_case_globals)]
            #[unsafe(link_section = "__DATA,__interpose")]
            static mut _interpose_entry: InterposeEntry = InterposeEntry { _new: hook_fn as *const c_void, _old: $target as *const c_void };
        };
    };
}

unsafe extern "C" {
    #[link_name = "open"]
    fn libc_open(path: *const c_char, flags: c_int, args: VaList) -> c_int;
}

const _: () = {

    unsafe extern "C" fn open_interposed(path_ptr: *const c_char, flags: c_int, mut args: ...) -> c_int {
        let path = Path::new(OsStr::from_bytes(unsafe { CStr::from_ptr(path_ptr) }.to_bytes()));



        eprintln!("opening {:?}", path);

        let mut addrs = [null_mut::<c_void>(); 2];
        let backtrace_len = unsafe { libc::backtrace(addrs.as_mut_ptr(), addrs.len() as _) } as usize;
        if backtrace_len == 2 {
            let caller_addr = addrs[1];

            let mut dl_info: libc::Dl_info = unsafe { zeroed() };
            let ret = unsafe { libc::dladdr(caller_addr, &mut dl_info) };
            let dl_path = OsStr::from_bytes(if ret == 0 {
                b"(not found)"
            } else {
                unsafe { CStr::from_ptr(dl_info.dli_fname) }.to_bytes()
            });

            eprintln!("\t{}", Path::new(dl_path).display());
        }
        // https://github.com/rust-lang/rust/issues/44930
        // https://github.com/thepowersgang/va_list-rs/
        // https://github.com/mstange/samply/blob/02a7b3771d038fc5c9226fd0a6842225c59f20c1/samply-mac-preload/src/lib.rs#L85-L93
        if flags & libc::O_CREAT != 0 {
            let mode: libc::c_int = unsafe { args.arg() };
            // https://github.com/tailhook/openat/issues/21#issuecomment-535914957
            unsafe { libc::open(path_ptr, flags, mode) }
        } else {
            unsafe { libc::open(path_ptr, flags) }
        }
    }


    #[used]
    #[allow(dead_code)]
    #[allow(non_upper_case_globals)]
    #[unsafe(link_section = "__DATA,__interpose")]
    static mut _interpose_entry: InterposeEntry = InterposeEntry { _new: libc::open as *const c_void, _old: open_interposed as *const c_void };
};

interpose! {
    libc::execve => unsafe extern "C" fn(
        prog: *const libc::c_char,
        argv: *const *const libc::c_char,
        envp: *const *const libc::c_char,
    ) -> libc::c_int {
        eprintln!("execve");
        let bump = Bump::new();
        let mut raw_cmd =  RawCommand { prog: prog.cast(), argv: argv.cast(), envp: envp.cast() };
        if let Err(err) = unsafe { CLIENT.interpose_command(&bump, &mut raw_cmd) } {
            err.set();
            return -1;
        }
        unsafe {
            libc::execve(raw_cmd.prog.cast(), raw_cmd.argv.cast(), raw_cmd.envp.cast())
        }
    }
}

interpose! {
    libc::posix_spawn => unsafe extern "C" fn(
        pid: *mut libc::pid_t,
        path: *const c_char,
        file_actions: *const libc::posix_spawn_file_actions_t,
        attrp: *const libc::posix_spawnattr_t,
        argv: *const *mut c_char,
        envp: *const *mut c_char,
    ) -> libc::c_int {
        let bump = Bump::new();
        let mut raw_cmd = RawCommand { prog: path, argv: argv.cast(), envp: envp.cast() };

        if let Err(err) = unsafe { CLIENT.interpose_command(&bump, &mut raw_cmd) } {
            return err as c_int;
        }


        unsafe {
            libc::posix_spawn(pid, raw_cmd.prog, file_actions, attrp, raw_cmd.argv.cast(), raw_cmd.envp.cast())
        }
    }
}


interpose!(
    libc::posix_spawnp => unsafe extern "C" fn(
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

        let mut raw_cmd =  RawCommand { prog: file, argv: argv.cast(), envp: envp.cast() };
        if let Err(err) = unsafe { CLIENT.interpose_command(&bump, &mut raw_cmd) } {
            return err as c_int;
        }
        unsafe {
            libc::posix_spawnp(pid, raw_cmd.prog, file_actions, attrp, raw_cmd.argv.cast(), raw_cmd.envp.cast())
        }
    }
);
