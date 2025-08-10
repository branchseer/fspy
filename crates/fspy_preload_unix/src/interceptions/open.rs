use std::ffi::CStr;

use libc::FILE;

use crate::{
    client::{
        convert::{ModeStr, OpenFlags, PathAt},
        global_client, handle_open,
    },
    libc::{c_char, c_int},
    macros::intercept,
};

fn has_mode_arg(o_flags: c_int) -> bool {
    if o_flags & libc::O_CREAT != 0 {
        return true;
    };
    #[cfg(target_os = "linux")]
    if o_flags & libc::O_TMPFILE != 0 {
        return true;
    }
    false
}

intercept!(open(64): unsafe extern "C" fn(*const c_char, c_int, args: ...) -> c_int);
unsafe extern "C" fn open(path: *const c_char, flags: c_int, mut args: ...) -> c_int {
    unsafe { handle_open(path, OpenFlags(flags)) };
    if has_mode_arg(flags) {
        #[cfg(not(target_os = "macos"))]
        type Mode = libc::mode_t;
        #[cfg(target_os = "macos")] // https://github.com/tailhook/openat/issues/21#issuecomment-535914957
        type Mode = c_int;

        let mode: Mode = unsafe { args.arg() };
        unsafe { open::original()(path, flags, mode) }
    } else {
        unsafe { open::original()(path, flags) }
    }
}

intercept!(openat(64): unsafe extern "C" fn(c_int, *const c_char, c_int, ...) -> c_int);
unsafe extern "C" fn openat(
    dirfd: c_int,
    path_ptr: *const c_char,
    flags: c_int,
    mut args: ...
) -> c_int {
    unsafe { handle_open(PathAt(dirfd, path_ptr), OpenFlags(flags)) };

    if has_mode_arg(flags) {
        // https://github.com/tailhook/openat/issues/21#issuecomment-535914957
        let mode: libc::c_int = unsafe { args.arg() };
        unsafe { openat::original()(dirfd, path_ptr, flags, mode) }
    } else {
        unsafe { openat::original()(dirfd, path_ptr, flags) }
    }
}

intercept!(fopen(64): unsafe extern "C" fn(path: *const c_char, mode: *const c_char) -> *mut FILE);
unsafe extern "C" fn fopen(path: *const c_char, mode: *const c_char) -> *mut libc::FILE {
    unsafe { handle_open(path, ModeStr(mode)) };
    unsafe { fopen::original()(path, mode) }
}

intercept!(freopen(64): unsafe extern "C" fn(path: *const c_char, mode: *const c_char, stream: *mut FILE) -> *mut FILE);
unsafe extern "C" fn freopen(
    path: *const c_char,
    mode: *const c_char,
    stream: *mut FILE,
) -> *mut FILE {
    unsafe { handle_open(path, ModeStr(mode)) };
    unsafe { freopen::original()(path, mode, stream) }
}
