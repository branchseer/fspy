pub mod handler;
mod listener;

use std::{
    io,
    os::fd::{FromRawFd, OwnedFd},
};

pub use handler::SeccompNotifyHandler;
use listener::NotifyListener;
use passfd::tokio::FdPassingExt as _;
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter};
use tokio::{net::UnixStream, task::JoinSet};
use tracing::{Level, span};

use crate::{
    bindings::alloc::alloc_seccomp_notif_resp,
    payload::{Filter, Payload},
};

pub fn supervise<H: SeccompNotifyHandler + Default + Send + 'static>()
-> io::Result<(Payload, impl Future<Output = io::Result<Vec<H>>>)> {
    let (notify_fd_receiver, notify_fd_sender) = UnixStream::pair()?;
    let notify_fd_sender = notify_fd_sender.into_std()?;
    notify_fd_sender.set_nonblocking(false)?;

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

    let filter = Filter(
        BpfProgram::try_from(filter)
            .unwrap()
            .into_iter()
            .map(|sock_filter| sock_filter.into())
            .collect(),
    );

    let payload = Payload {
        ipc_fd: notify_fd_sender.into(),
        filter,
    };

    let handle_loop = async move {
        let mut join_set: JoinSet<io::Result<H>> = JoinSet::new();

        loop {
            let notify_fd = match notify_fd_receiver.recv_fd().await {
                Ok(raw_fd) => unsafe { OwnedFd::from_raw_fd(raw_fd) },
                Err(err) => {
                    return if err.kind() == io::ErrorKind::UnexpectedEof {
                        let mut handlers = Vec::<H>::new();
                        while let Some(handler) = join_set.join_next().await.transpose()? {
                            handlers.push(handler?);
                        }
                        Ok(handlers)
                    } else {
                        Err(err)
                    };
                }
            };

            let mut listener = NotifyListener::try_from(notify_fd)?;

            let mut handler = H::default();
            let mut resp_buf = alloc_seccomp_notif_resp();

            join_set.spawn(async move {
                while let Some(notify) = listener.next().await? {
                    let _span = span!(Level::TRACE, "notify loop tick");
                    // Errors on the supervisor side shouldn't block the syscall.
                    let handle_result = handler.handle_notify(notify);
                    let notify_id = notify.id;
                    listener.send_continue(notify_id, &mut resp_buf)?;
                    handle_result?;
                }
                io::Result::Ok(handler)
            });
        }
    };
    Ok((payload, handle_loop))
}
