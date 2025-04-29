mod fixture;

use std::path::PathBuf;

#[cfg(target_os = "linux")]
#[path ="./linux/mod.rs"]
mod os_impl;

#[cfg(target_os = "macos")]
#[path ="./macos/mod.rs"]
mod os_impl;

mod command_builder;

pub struct FileSystemAccess {
    pub path: PathBuf
}

pub use os_impl::Spy;
