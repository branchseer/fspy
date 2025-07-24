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
use tracing::{Level, instrument, span, trace};

use crate::bindings::{
    NotifyListener,
    alloc::{Alloced, alloc_seccomp_notif_resp},
};

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

        let _span = span!(Level::TRACE, "notify loop");
        let loop_handle = tokio::spawn(async move {
            let mut handler = H::default();
            let listener = bindings::NotifyListener::try_from(notify_fd)?;
            let mut notify_buf = alloc_seccomp_notif();
            let mut resp_buf = alloc_seccomp_notif_resp();
            while let Some(notify) = listener.next(&mut notify_buf).await? {
                let _span = span!(Level::TRACE, "notify loop tick");
                // Errors on the supervisor side shouldn't block the syscall.
                let handle_result = handler.handle_notify(notify);
                listener.send_continue(notify.id, &mut resp_buf)?;
                handle_result?;
            }
            io::Result::Ok(handler)
        });

        let handler = loop_handle.await??;

        Ok(vec![handler])
    })
}
