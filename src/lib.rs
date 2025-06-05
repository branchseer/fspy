#![cfg_attr(target_os = "windows", feature(windows_process_extensions_main_thread_handle))]

mod fixture;
mod consts;

#[cfg(target_os = "macos")]
mod shebang;

use std::path::PathBuf;

#[cfg(target_os = "linux")]
#[path ="./linux/mod.rs"]
mod os_impl;

#[cfg(target_os = "macos")]
#[path ="./macos/mod.rs"]
mod os_impl;

#[cfg(target_os = "windows")]
#[path ="./windows/mod.rs"]
mod os_impl;

mod command_builder;

pub struct FileSystemAccess {
    pub path: PathBuf
}

pub use os_impl::*;

pub use consts::{ AccessMode, PathAccess };
