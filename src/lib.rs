#![cfg_attr(
    target_os = "windows",
    feature(windows_process_extensions_main_thread_handle)
)]
#![feature(once_cell_try)]

mod fixture;

#[cfg(unix)]
mod unix;

#[cfg(target_os = "linux")]
#[path = "./linux/mod.rs"]
mod os_impl;

#[cfg(target_os = "macos")]
#[path = "./macos/mod.rs"]
mod os_impl;

#[cfg(target_os = "windows")]
#[path = "./windows/mod.rs"]
mod os_impl;

mod command;

use std::{env::temp_dir, ffi::OsStr, fs::create_dir, io, sync::OnceLock};

pub use command::Command;
use futures_util::future::{BoxFuture, LocalBoxFuture};
use os_impl::SpyInner;

#[ouroboros::self_referencing]
pub struct PathAccesses {
    bump: bumpalo::Bump,
    #[borrows(bump)]
    #[covariant]
    accesses: &'this [PathAccess<'this>],
}

impl PathAccesses {
    pub fn as_slice(&self) -> &[PathAccess<'_>] {
        self.borrow_accesses()
    }
}

pub struct TrackedChild {
    pub tokio_child: Child,
    pub accesses_future: LocalBoxFuture<'static, io::Result<PathAccesses>>,
}

pub struct Spy(SpyInner);
impl Spy {
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> io::Result<Self> {
        let tmp_dir = temp_dir().join("fspy");
        let _ = create_dir(&tmp_dir);
        Ok(Self(SpyInner::init_in_dir(&tmp_dir)?))
    }
    #[cfg(target_os = "linux")]
    pub fn new() -> io::Result<Self> {
        Ok(Self(SpyInner::init()?))
    }
    pub fn global() -> io::Result<&'static Self> {
        static GLOBAL_SPY: OnceLock<Spy> = OnceLock::new();
        GLOBAL_SPY.get_or_try_init(|| Self::new())
    }
    pub fn new_command<S: AsRef<OsStr>>(&self, program: S) -> Command {
        Command {
            program: program.as_ref().to_os_string(),
            envs: Default::default(),
            args: vec![],
            cwd: None,
            #[cfg(unix)]
            arg0: None,
            spy_inner: self.0.clone(),
            stderr: None,
            stdout: None,
            stdin: None,
        }
    }
}

pub use fspy_shared::ipc::*;
use tokio::process::Child;
