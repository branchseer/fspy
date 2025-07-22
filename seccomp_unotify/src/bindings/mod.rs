mod listener;
pub mod alloc;

use seccompiler::BpfProgramRef;
use std::{
    io,
    os::{
        fd::{FromRawFd, OwnedFd},
        raw::c_int,
    },
};
use syscalls::{Sysno, syscall};

pub unsafe fn seccomp(
    operation: libc::c_uint,
    flags: libc::c_uint,
    args: *mut libc::c_void,
) -> io::Result<libc::c_int> {
    let ret = syscall!(Sysno::seccomp, operation, flags, args)?;
    Ok(c_int::try_from(ret).unwrap())
}

pub fn listen_unotify_with_filter(prog: BpfProgramRef) -> io::Result<OwnedFd> {
    let mut filter = libc::sock_fprog {
        len: prog.len().try_into().unwrap(),
        filter: prog.as_ptr().cast_mut().cast(),
    };

    let fd = unsafe {
        seccomp(
            libc::SECCOMP_SET_MODE_FILTER,
            libc::SECCOMP_FILTER_FLAG_NEW_LISTENER as _,
            (&raw mut filter).cast(),
        )?
    };

    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}
pub use listener::NotifyListener;
