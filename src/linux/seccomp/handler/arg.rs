use std::io;

use arrayvec::ArrayVec;
use libc::seccomp_notif;

trait FromSyscallArg: Sized {
    fn from_syscall_arg(pid: u32, arg: u64) -> io::Result<Self>;
}

#[derive(Debug)]
pub struct CStrPtr<const MAX_READ: usize>(ArrayVec<u8, MAX_READ>);

impl<const MAX_READ: usize> CStrPtr<MAX_READ> {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl<const MAX_READ: usize> FromSyscallArg for CStrPtr<MAX_READ> {
    fn from_syscall_arg(pid: u32, arg: u64) -> io::Result<Self> {
        let mut buf = ArrayVec::<u8, MAX_READ>::new();
        let local_iov = libc::iovec {
            iov_base: buf.as_mut_ptr().cast(),
            iov_len: MAX_READ,
        };

        let remote_iov = libc::iovec {
            iov_base: arg as _,
            iov_len: MAX_READ,
        };

        // TODO: handle partitial read
        let read_size = unsafe {
            libc::process_vm_readv(pid.try_into().unwrap(), &local_iov, 1, &remote_iov, 1, 0)
        };

        let Ok(read_size) = usize::try_from(read_size) else {
            return Err(io::Error::last_os_error());
        };

        unsafe { buf.set_len(read_size) };
        Ok(Self(buf))
    }
}

#[derive(Debug)]
pub struct Ignored(());
impl FromSyscallArg for Ignored {
    fn from_syscall_arg(pid: u32, arg: u64) -> io::Result<Self> {
        Ok(Ignored(()))
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
