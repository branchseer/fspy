use std::{os::fd::OwnedFd, path::Path};

use nix::{fcntl::{open, OFlag}, sys::stat::{fstat, Mode}};

pub fn open_executable(path: impl AsRef<Path>) -> nix::Result<OwnedFd> {
    let fd = open(path.as_ref(),  OFlag::O_RDONLY | OFlag::O_CLOEXEC, Mode::empty())?;
    let stat = fstat(&fd)?;
    let mode = Mode::from_bits_retain(stat.st_mode);
    if !mode.contains(Mode::S_IXUSR | Mode::S_IXGRP | Mode::S_IXOTH) {
        return Err(nix::Error::EACCES);
    };
    Ok(fd)
}
