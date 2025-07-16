pub mod inject;

use std::{
    ffi::{OsStr, OsString},
    os::{
        fd::RawFd,
        unix::ffi::{OsStrExt, OsStringExt as _},
    },
};

use bincode::{Decode, Encode};

use crate::ipc::{NativeString};

pub const PAYLOAD_ENV_NAME: &str = "FSPY_PAYLOAD";

#[derive(Debug, Encode, Decode, Clone)]
pub struct Fixtures {
    pub bash_path: NativeString,
    pub coreutils_path: NativeString,
    pub interpose_cdylib_path: NativeString,
}

#[derive(Debug, Encode, Decode, Clone)]
pub struct Payload {
    pub ipc_fd: RawFd,
    pub fixtures: Fixtures,
}
