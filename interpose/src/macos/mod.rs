mod caller;
mod client;

mod interpose_macros;

use std::{
    borrow::Cow,
    env::current_dir,
    ffi::{CStr, OsStr},
    os::{fd::BorrowedFd, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
};

use bstr::BStr;
use bumpalo::Bump;
use caller::caller_dli_fname;
use client::{CLIENT, RawCommand};
use interpose_macros::interpose_libc;
use libc::{c_char, c_int};
use nix::fcntl::FcntlArg;

use fspy_shared::ipc::AccessMode;

unsafe fn handle_open(dirfd: c_int, path_ptr: *const c_char, flags: c_int) {
    let path = Path::new(OsStr::from_bytes(
        unsafe { CStr::from_ptr(path_ptr) }.to_bytes(),
    ));

    let path: Cow<'_, Path> = if path.is_absolute() {
        Cow::Borrowed(path)
    } else {
        let mut dir = PathBuf::new();
        if dirfd == libc::AT_FDCWD {
            dir = current_dir().unwrap()
        } else {
            nix::fcntl::fcntl(
                unsafe { BorrowedFd::borrow_raw(dirfd) },
                FcntlArg::F_GETPATH(&mut dir),
            )
            .unwrap();
        };
        dir.push(path);
        Cow::Owned(dir)
    };

    let acc_mode = flags & libc::O_ACCMODE;
    CLIENT.send(
        if acc_mode == libc::O_RDWR {
            AccessMode::ReadWrite
        } else if acc_mode == libc::O_WRONLY {
            AccessMode::Write
        } else {
            AccessMode::Read
        },
        path.as_os_str().as_bytes().into(),
    );
}

unsafe extern "C" fn open(path_ptr: *const c_char, flags: c_int, mut args: ...) -> c_int {
    unsafe { handle_open(libc::AT_FDCWD, path_ptr, flags) };

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
interpose_libc!(open);

unsafe extern "C" fn openat(
    dirfd: c_int,
    path_ptr: *const c_char,
    flags: c_int,
    mut args: ...
) -> c_int {
    unsafe { handle_open(dirfd, path_ptr, flags) };
    if flags & libc::O_CREAT != 0 {
        let mode: libc::c_int = unsafe { args.arg() };
        unsafe { libc::openat(dirfd, path_ptr, flags, mode) }
    } else {
        unsafe { libc::openat(dirfd, path_ptr, flags) }
    }
}

interpose_libc!(openat);

unsafe extern "C" fn opendir(dirname: *const c_char) -> *mut libc::DIR {
    unsafe { handle_open(libc::AT_FDCWD, dirname, libc::O_RDONLY) };
    unsafe { libc::opendir(dirname) }
}
interpose_libc!(opendir);


unsafe extern "C" fn lstat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    unsafe { handle_open(libc::AT_FDCWD, path, libc::O_RDONLY) };
    unsafe { libc::lstat(path, buf) }
}
interpose_libc!(lstat);


unsafe extern "C" fn stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    unsafe { handle_open(libc::AT_FDCWD, path, libc::O_RDONLY) };
    unsafe { libc::stat(path, buf) }
}

interpose_libc!(stat);

unsafe extern "C" fn fstatat(dirfd: c_int, pathname: *const c_char, buf: *mut libc::stat, flags: c_int) -> c_int {
    unsafe { handle_open(dirfd, pathname, libc::O_RDONLY) };
    unsafe { libc::fstatat(dirfd, pathname, buf, flags) }
}
interpose_libc!(fstatat);

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
