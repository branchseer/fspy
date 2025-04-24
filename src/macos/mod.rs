use std::{ffi::OsStr, io, os::fd::OwnedFd};

use futures_util::{stream::poll_fn, Stream, TryStream};
use tokio::{net::UnixDatagram, process::Command};

use crate::FileSystemAccess;

pub struct Spy {

}

impl Spy {
    pub fn init() -> io::Result<Self> {
        Ok(Self {  })
    }
    pub fn new_command<S: AsRef<OsStr>>(
        &self,
        program: S,
        config_command: impl FnOnce(&mut Command) -> io::Result<()>,
    ) -> io::Result<(
        Command,
        impl Stream<Item = io::Result<FileSystemAccess>>
            + Send
            + Sync
            + 'static,
    )> {
        let mut command = Command::new(program);
        config_command(&mut command)?;

        let (receiver, sender) = UnixDatagram::pair()?;
        let sender = OwnedFd::from(sender.into_std());

        Ok((command, poll_fn(|_| {
            todo!()
        })))
    }
}
