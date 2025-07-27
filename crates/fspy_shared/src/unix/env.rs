use std::{ffi::{OsStr, OsString}, os::unix::ffi::{OsStrExt as _, OsStringExt as _}};

use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine as _};
use bincode::{Decode, Encode};

use crate::ipc::BINCODE_CONFIG;

pub fn encode_env<T: Encode>(value: &T) -> OsString {
    let bincode_bytes = bincode::encode_to_vec(value, BINCODE_CONFIG).unwrap();
    OsString::from_vec(Vec::<u8>::from(
        BASE64_STANDARD_NO_PAD.encode(&bincode_bytes),
    ))
}

pub fn decode_env<T: Decode<()>>(os_str: &OsStr) -> T {
    let bincode_bytes = BASE64_STANDARD_NO_PAD.decode(os_str.as_bytes()).unwrap();
    let (payload, n) = bincode::decode_from_slice(&bincode_bytes, BINCODE_CONFIG).unwrap();
    assert_eq!(bincode_bytes.len(), n);
    payload
}
