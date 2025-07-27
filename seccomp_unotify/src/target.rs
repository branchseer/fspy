use std::{io, os::{fd::AsRawFd, unix::net::UnixStream}};

use libc::sock_filter;
use nix::sys::prctl::set_no_new_privs;
use passfd::FdPassingExt;

use crate::{bindings::install_unotify_filter, payload::Payload};

pub fn install_target(payload: Payload) -> io::Result<()> {
    set_no_new_privs()?;
    let sock_filters = payload
        .filter
        .0
        .into_iter()
        .map(sock_filter::from)
        .collect::<Vec<sock_filter>>();
    let notify_fd = install_unotify_filter(&sock_filters)?;
    let notify_sender = UnixStream::from(payload.ipc_fd);
    notify_sender.send_fd(notify_fd.as_raw_fd())?;
    Ok(())
}
