use libc::{seccomp_notif, seccomp_notif_resp};
use nix::{fcntl::FcntlArg, poll::{PollFd, PollFlags, PollTimeout}};
use tracing::trace;
use std::{
    io, net::TcpStream, os::fd::{AsFd, AsRawFd, OwnedFd}
};

use super::alloc::{Alloced};

pub struct NotifyListener {
    fd: OwnedFd,
}

impl TryFrom<OwnedFd> for NotifyListener {
    type Error = io::Error;
    fn try_from(fd: OwnedFd) -> Result<Self, Self::Error> {
        let mut nonblocking = true as libc::c_int;
        let ret = unsafe { libc::ioctl(fd.as_raw_fd(), libc::FIONBIO, &mut nonblocking) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            fd
        })
    }
}
// impl AsFd for NotifyListener {
//     fn as_fd(&self) -> BorrowedFd<'_> {
//         self.async_fd.as_fd()
//     }
// }

const SECCOMP_IOCTL_NOTIF_SEND: libc::c_ulong = 3222806785;
const SECCOMP_IOCTL_NOTIF_RECV: libc::c_ulong = 3226476800;
const SECCOMP_IOCTL_NOTIF_ID_VALID: libc::c_ulong = 1074274562;

impl NotifyListener {
    pub fn send_continue(
        &self,
        req_id: u64,
        buf: &mut Alloced<seccomp_notif_resp>,
    ) -> io::Result<()> {
        let resp = buf.zeroed();
        resp.id = req_id;
        resp.flags = libc::SECCOMP_USER_NOTIF_FLAG_CONTINUE as _;

        let ret = unsafe {
            libc::ioctl(
                self.fd.as_raw_fd(),
                SECCOMP_IOCTL_NOTIF_SEND,
                &raw mut *resp,
            )
        };
        if ret < 0 {
            let err = nix::Error::last();
            // ignore error if target process's syscall was interrupted
            if err == nix::Error::ENOENT {
                return Ok(());
            };
            return Err(err.into());
        };
        Ok(())
    }
    // Awaiting readable on AsyncFd doesn't work on older kernels like the one in Ubuntu 22.04 or WSL2.
    // Let's stick to the blocking approach for now
    pub fn next<'a>(
        &self,
        buf: &'a mut Alloced<seccomp_notif>,
    ) -> io::Result<Option<&'a seccomp_notif>> {
        let notif_buf = buf.zeroed();
        let mut fds = [ PollFd::new(self.fd.as_fd(), PollFlags::POLLIN) ];

        loop {
            trace!("polling notify fd");
            nix::poll::poll(&mut fds, PollTimeout::NONE)?;
            trace!("fd polled: {:?}", fds[0].revents());

            if let Some(events) = fds[0].revents() && events.contains(PollFlags::POLLHUP) {
                return Ok(None);
            };
            trace!("SECCOMP_IOCTL_NOTIF_RECV");
            let ret = unsafe {
                libc::ioctl(self.fd.as_raw_fd(), SECCOMP_IOCTL_NOTIF_RECV, &raw mut *notif_buf)
            };
            trace!("SECCOMP_IOCTL_NOTIF_RECV returns {}", ret);
            if ret < 0 {
                let err = nix::Error::last();
                trace!("SECCOMP_IOCTL_NOTIF_RECV error: {:?}", err);
                match err {
                    nix::Error::EINTR | nix::Error::EWOULDBLOCK => continue,
                    nix::Error::ENOENT => return Ok(None),
                    other => return Err(other.into()),
                }
            }
            return Ok(Some(notif_buf))
        }
    }
}
