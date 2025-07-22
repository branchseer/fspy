mod bindings;
pub mod handler;

use std::{
    ffi::CStr,
    io,
    mem::zeroed,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        raw::c_void,
    },
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{ScopedJoinHandle, available_parallelism},
};

use alloc::{alloc_seccomp_notif, alloc_seccomp_notif_resp};
pub use bindings::*;
use libc::{SECCOMP_GET_NOTIF_SIZES, c_int};
use tokio::{
    io::{unix::AsyncFd, Interest}, net::UnixStream, sync::Semaphore
};

// pub fn install_handler<H: handler::SeccompNotifyHandler>(
//     command: &mut Command,
//     handler: H,
// ) -> io::Result<impl Future<Output = io::Result<()>>> {
//     let (fd_sender, fd_receiver) = UnixStream::pair()?;
//     async move {
//         handler.handle_notify(todo!())?;
//         Ok(())
//     }
// }

pub async fn handle_unotify_fd(notify_fd: OwnedFd) -> io::Result<()> {
    let mut listener = bindings::NotifyListener::try_from(notify_fd)?;

    let path_cnt = AtomicUsize::new(0);
    let path_cnt = &path_cnt;

    let mut path_buf = [0u8; libc::PATH_MAX as usize];

    let mut request_cnt = 0u32;
    let semaphore = Arc::new(Semaphore::new(0));

    let mut notify_buf = bindings::alloc::alloc_seccomp_notif();
    let mut resp_buf = bindings::alloc::alloc_seccomp_notif_resp();

    while let Some(notify) = listener.next(&mut notify_buf).await? {
        let x = notify.data.nr == const { syscalls::Sysno::openat as _ };
        let path_remote_ptr = if libc::c_long::from(notify.data.nr) == libc::SYS_openat {
            notify.data.args[1]
        } else {
            notify.data.args[0]
        };

        let local_iov = libc::iovec {
            iov_base: path_buf.as_mut_ptr().cast(),
            iov_len: path_buf.len(),
        };

        let remote_iov = libc::iovec {
            iov_base: path_remote_ptr as _,
            iov_len: path_buf.len(),
        };

        let read_size =
            unsafe { libc::process_vm_readv(notify.pid as _, &local_iov, 1, &remote_iov, 1, 0) };
        let Ok(read_size) = usize::try_from(read_size) else {
            let err = io::Error::last_os_error();

            if err.raw_os_error() == Some(libc::ESRCH) {
                // the process is terminated
                return Ok(());
            } else {
                return Err(err);
            };
        };

        listener.send_continue(notify.id, &mut resp_buf)?;
        let path = CStr::from_bytes_until_nul(&path_buf[..read_size])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        dbg!(path);
    }
    dbg!(request_cnt);
    Ok(())
}
