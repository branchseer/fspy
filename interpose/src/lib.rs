mod shebang;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[link(name = "fspy_do_not_build_this_cydlib")]
unsafe extern { }
