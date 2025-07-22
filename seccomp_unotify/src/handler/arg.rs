use std::{
    ffi::{CStr, OsString},
    io,
    os::{fd::RawFd, raw::c_void},
};

use arrayvec::ArrayVec;
use libc::seccomp_notif;

pub trait FromSyscallArg: Sized {
    fn from_syscall_arg(pid: u32, arg: u64) -> io::Result<Self>;
}

#[derive(Debug)]
pub struct CStrPtr {
    pid: u32,
    remote_ptr: *mut c_void,
}

impl CStrPtr {
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let local_iov = libc::iovec {
            iov_base: buf.as_mut_ptr().cast(),
            iov_len: buf.len(),
        };

        let remote_iov = libc::iovec {
            iov_base: self.remote_ptr,
            iov_len: buf.len(),
        };

        // TODO: loop partitial read
        let read_size = unsafe {
            libc::process_vm_readv(
                self.pid.try_into().unwrap(),
                &local_iov,
                1,
                &remote_iov,
                1,
                0,
            )
        };

        let Ok(read_size) = usize::try_from(read_size) else {
            return Err(io::Error::last_os_error());
        };

        let cstr = CStr::from_bytes_until_nul(&buf[..read_size])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        Ok(cstr.count_bytes())
    }
}

impl FromSyscallArg for CStrPtr {
    fn from_syscall_arg(pid: u32, arg: u64) -> io::Result<Self> {
        Ok(Self {
            pid,
            remote_ptr: arg as _,
        })
    }
}

#[derive(Debug)]
pub struct Ignored(());
impl FromSyscallArg for Ignored {
    fn from_syscall_arg(_pid: u32, _arg: u64) -> io::Result<Self> {
        Ok(Ignored(()))
    }
}

pub struct Fd {
    pid: u32,
    fd: RawFd,
}
impl FromSyscallArg for Fd {
    fn from_syscall_arg(pid: u32, arg: u64) -> io::Result<Self> {
        Ok(Self { pid, fd: arg as _ })
    }
}

impl Fd {
    pub fn get_path(&self) -> nix::Result<OsString> {
        nix::fcntl::readlink(
            if self.fd == libc::AT_FDCWD {
                format!("/proc/{}/cwd", self.pid)
            } else {
                format!("/proc/{}/fd/{}", self.pid, self.fd)
            }
            .as_str(),
        )
    }
}

pub trait FromNotify: Sized {
    fn from_notify(notif: &seccomp_notif) -> io::Result<Self>;
}

impl<T: FromSyscallArg> FromNotify for (T,) {
    fn from_notify(notif: &seccomp_notif) -> io::Result<Self> {
        Ok((T::from_syscall_arg(notif.pid, notif.data.args[0])?,))
    }
}

impl<T1: FromSyscallArg, T2: FromSyscallArg> FromNotify for (T1, T2) {
    fn from_notify(notif: &seccomp_notif) -> io::Result<Self> {
        Ok((
            T1::from_syscall_arg(notif.pid, notif.data.args[0])?,
            T2::from_syscall_arg(notif.pid, notif.data.args[1])?,
        ))
    }
}

impl<T1: FromSyscallArg, T2: FromSyscallArg, T3: FromSyscallArg> FromNotify for (T1, T2, T3) {
    fn from_notify(notif: &seccomp_notif) -> io::Result<Self> {
        Ok((
            T1::from_syscall_arg(notif.pid, notif.data.args[0])?,
            T2::from_syscall_arg(notif.pid, notif.data.args[1])?,
            T3::from_syscall_arg(notif.pid, notif.data.args[2])?,
        ))
    }
}

impl<T1: FromSyscallArg, T2: FromSyscallArg, T3: FromSyscallArg, T4: FromSyscallArg> FromNotify
    for (T1, T2, T3, T4)
{
    fn from_notify(notif: &seccomp_notif) -> io::Result<Self> {
        Ok((
            T1::from_syscall_arg(notif.pid, notif.data.args[0])?,
            T2::from_syscall_arg(notif.pid, notif.data.args[1])?,
            T3::from_syscall_arg(notif.pid, notif.data.args[2])?,
            T4::from_syscall_arg(notif.pid, notif.data.args[3])?,
        ))
    }
}
