use std::os::fd::RawFd;

use bincode::{Decode, Encode};

pub mod nul_term;
pub mod inject;

pub const PAYLOAD_ENV_NAME: &str = "FSPY_PAYLOAD";
pub const EXECVE_HOST_NAME: &str = "fspy_execve_host";

#[derive(Debug, Encode, Decode)]
pub struct Payload {
    pub preload_lib_path: String,
    pub ipc_fd: RawFd,
    pub bootstrap: bool,
}
