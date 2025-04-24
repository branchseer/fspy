// the bin target is only for linux.
// For non-linux targets, mark the bin target as `no_main` so that
// `cargo check` and rust-analyser won't complain, whereas build can
// fail with "_main not found" error
#![cfg_attr(not(target_os = "linux"), no_main)]

#[cfg(target_os = "linux")]
mod linux_execve_host;

#[cfg(target_os = "linux")]
fn main() -> ! {
    linux_execve_host::main()
}
