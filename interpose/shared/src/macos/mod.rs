pub mod inject;

use std::{
    ffi::{OsStr, OsString},
    os::{
        fd::RawFd,
        unix::ffi::{OsStrExt, OsStringExt as _},
    },
};

use base64::prelude::*;
use bincode::{Decode, Encode};

use crate::ipc::{BINCODE_CONFIG, NativeString};

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

pub fn encode_payload(payload: &Payload) -> OsString {
    let bincode_bytes = bincode::encode_to_vec(payload, BINCODE_CONFIG).unwrap();
    OsString::from_vec(Vec::<u8>::from(
        BASE64_STANDARD_NO_PAD.encode(&bincode_bytes),
    ))
}

pub fn decode_payload(os_str: &OsStr) -> Payload {
    let bincode_bytes = BASE64_STANDARD_NO_PAD.decode(os_str.as_bytes()).unwrap();
    let (payload, n) = bincode::decode_from_slice(&bincode_bytes, BINCODE_CONFIG).unwrap();
    assert_eq!(bincode_bytes.len(), n);
    payload
}
