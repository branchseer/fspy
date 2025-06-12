pub mod ipc;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;
