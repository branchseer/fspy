use std::{ffi::OsStr, io, os::unix::ffi::OsStrExt};

use super::seccomp::handler::{arg::{Ignored, CStrPtr}, impl_handler};


const PATH_MAX: usize = libc::PATH_MAX as usize;
type CStrPath = CStrPtr<PATH_MAX>;



pub struct FsSyscallHandler;

impl FsSyscallHandler {
    fn openat(&self, (_, path): (Ignored, CStrPath)) -> io::Result<()> {
        dbg!(OsStr::from_bytes(path.as_bytes()));
        Ok(())
    }
}

impl_handler!(
    FsSyscallHandler,
    openat
);
