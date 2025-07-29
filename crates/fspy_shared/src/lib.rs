pub mod ipc;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;
