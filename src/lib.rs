#![cfg_attr(
    target_os = "windows",
    feature(windows_process_extensions_main_thread_handle)
)]

mod fixture;
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

use std::{env::temp_dir, ffi::OsStr, fs::create_dir, io};

pub use os_impl::Child;

pub use command::Command;
use os_impl::SpyInner;

pub struct Spy(SpyInner);
impl Spy {
    pub fn init() -> io::Result<Self> {
        let tmp_dir = temp_dir().join("fspy");
        let _ = create_dir(&tmp_dir);
        Ok(Self(SpyInner::init_in_dir(&tmp_dir)?))
    }
    pub fn new_command<S: AsRef<OsStr>>(&self, program: S) -> Command {
        Command {
            program: program.as_ref().to_os_string(),
            envs: std::env::vars_os().collect(),
            args: vec![],
            cwd: None,
            arg0: None,
            spy_inner: self.0.clone(),
        }
    }
}

pub use fspy_shared::ipc::*;
