use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
    thread::available_parallelism,
};

use libc::seccomp_notif_resp;
use nix::sys::prctl::set_no_new_privs;
use passfd::{FdPassingExt as _, tokio::FdPassingExt as _};
use seccompiler::{BpfProgram, BpfProgramRef, SeccompAction, SeccompFilter};
use tokio::{
    net::UnixStream,
    process::Command,
    task::{JoinHandle, JoinSet},
};

use bindings::alloc::alloc_seccomp_notif;
use tracing::{instrument, span, trace, Level};

use crate::bindings::{alloc::{alloc_seccomp_notif_resp, Alloced}, NotifyListener};

mod bindings;
pub mod handler;

#[instrument]
pub fn install_handler<H: handler::SeccompNotifyHandler + Send + Default + 'static>(
    cmd: &mut Command,
) -> io::Result<impl Future<Output = io::Result<Vec<H>>> + Send + use<H>> {
    let (fd_receiver, fd_sender) = UnixStream::pair()?;
    let fd_sender = fd_sender.into_std()?;
    fd_sender.set_nonblocking(false)?;
    unsafe {
        cmd.pre_exec(move || {
            set_no_new_privs()?;
            let filter = SeccompFilter::new(
                H::syscalls()
                    .iter()
                    .map(|sysno| (sysno.id().into(), vec![]))
                    .collect(),
                SeccompAction::Allow,
                SeccompAction::Raw(libc::SECCOMP_RET_USER_NOTIF),
                std::env::consts::ARCH.try_into().unwrap(),
            )
            .unwrap();

            let prog = BpfProgram::try_from(filter).unwrap();
            let notify_fd = bindings::listen_unotify_with_filter(&prog)?;
            fd_sender.send_fd(notify_fd.as_raw_fd())?;
            Ok(())
        })
    };
    Ok(async move {
        let _span = span!(Level::TRACE, "supervisor task");

        let notify_fd = fd_receiver.recv_fd().await?;

        trace!("notify received");

        let notify_fd = unsafe { OwnedFd::from_raw_fd(notify_fd) };

        let parallelism = 1; // available_parallelism()?.get();
        let mut join_set = JoinSet::<io::Result<H>>::new();
        // Tested with esbuild: the kernel load-balances notifications on at most cpu_num of notify_fd duplicates.
        for _ in 0..parallelism {
            let notify_fd = notify_fd.try_clone()?;
            let mut handler = H::default();
            join_set.spawn_blocking(move || {
                let _span = span!(Level::TRACE, "notify loop");
                let listener = bindings::NotifyListener::try_from(notify_fd)?;
                let mut notify_buf = alloc_seccomp_notif();
                let mut resp_buf = alloc_seccomp_notif_resp();
                while let Some(notify) = listener.next(&mut notify_buf)? {
                    let _span = span!(Level::TRACE, "notify loop tick");
                    // Errors on the supervisor side shouldn't block the syscall.
                    let handle_result = handler.handle_notify(notify);
                    listener.send_continue(notify.id, &mut resp_buf)?;
                    handle_result?
                }
                Ok(handler)
            });
        }

        let mut handlers = Vec::<H>::with_capacity(parallelism);
        while let Some(handler) = join_set.join_next().await.transpose()? {
            handlers.push(handler?);
        }
        Ok(handlers)
    })
}
