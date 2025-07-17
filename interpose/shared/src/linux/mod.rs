use std::os::fd::RawFd;

use bincode::{Decode, Encode};

use crate::ipc::NativeString;

pub mod nul_term;
pub mod inject;

pub const PAYLOAD_ENV_NAME: &str = "FSPY_PAYLOAD";
pub const EXECVE_HOST_NAME: &str = "fspy_execve_host";

#[derive(Debug, Encode, Decode)]
pub struct Payload {
    pub execve_host_path: NativeString,
    pub ipc_fd: RawFd,
    pub bootstrap: bool,
}
