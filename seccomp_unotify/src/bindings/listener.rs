use libc::{seccomp_notif, seccomp_notif_resp};
use tracing::trace;
use std::{
    io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
};

use super::alloc::{Alloced};
use tokio::io::unix::AsyncFd;

pub struct NotifyListener {
    async_fd: AsyncFd<OwnedFd>,
}

impl TryFrom<OwnedFd> for NotifyListener {
    type Error = io::Error;
    fn try_from(value: OwnedFd) -> Result<Self, Self::Error> {
        Ok(Self {
            async_fd: AsyncFd::new(value)?,
        })
    }
}
impl AsFd for NotifyListener {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.async_fd.as_fd()
    }
}

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
                self.async_fd.as_raw_fd(),
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
    pub async fn next<'a>(
        &self,
        buf: &'a mut Alloced<seccomp_notif>,
    ) -> io::Result<Option<&'a seccomp_notif>> {
        let notif_buf = buf.zeroed();

        loop {
            let mut ready_guard = self.async_fd.readable().await?;
            let ready = ready_guard.ready();
            trace!("notify fd readable: {:?}", ready);
            if ready.is_read_closed() || ready.is_write_closed() {
                return Ok(None);
            }

            if !ready.is_readable() {
                continue;
            }
            // TODO: check why this call solves the issue that `is_read_closed || is_write_closed` is never true.
            ready_guard.clear_ready();

            let raw_notify_fd = ready_guard.get_inner().as_raw_fd();
            let ret = unsafe {
                libc::ioctl(raw_notify_fd, SECCOMP_IOCTL_NOTIF_RECV, &raw mut *notif_buf)
            };

            if ret < 0 {
                let err = nix::Error::last();
                match err {
                    nix::Error::EINTR | nix::Error::EWOULDBLOCK | nix::Error::ENOENT => continue,
                    other => return Err(other.into()),
                }
            }
            return Ok(Some(notif_buf));
        }
    }
}
