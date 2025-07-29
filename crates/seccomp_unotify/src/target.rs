use std::{io::{self, IoSlice}, os::fd::AsRawFd};

use libc::sock_filter;
use nix::sys::{
    prctl::set_no_new_privs,
    socket::{ControlMessage, MsgFlags, sendmsg},
};

use crate::{bindings::install_unotify_filter, payload::SeccompPayload};

pub fn install_target(payload: &SeccompPayload) -> nix::Result<()> {
    set_no_new_privs()?;
    let sock_filters = payload
        .filter
        .0
        .iter()
        .copied()
        .map(sock_filter::from)
        .collect::<Vec<sock_filter>>();
    let notify_fd = install_unotify_filter(&sock_filters)?;
    sendmsg(
        payload.ipc_fd,
        &[IoSlice::new(&[0])],
        &[ControlMessage::ScmRights(&[notify_fd.as_raw_fd()])],
        MsgFlags::empty(),
        Option::<&()>::None,
    )?;
    Ok(())
}
