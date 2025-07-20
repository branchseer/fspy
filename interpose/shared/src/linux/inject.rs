use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::Path,
};

use allocator_api2::alloc::Allocator;

use super::Payload;
use crate::unix::cmdinfo::{CommandInfo, ensure_env};

pub struct PayloadWithEncodedString {
    pub payload: Payload,
    pub payload_string: OsString,
}

pub fn inject<'a, A: Allocator + Copy + 'a>(
    _alloc: A,
    command: &mut CommandInfo<'a, A>,
    payload_with_encoded_str: &'a PayloadWithEncodedString,
) -> nix::Result<()> {
    ensure_env(
        &mut command.envs,
        OsStr::from_bytes(b"LD_PRELOAD"),
        payload_with_encoded_str
            .payload
            .preload_lib_path
            .as_str()
            .as_ref(),
    )?;
    Ok(())
}
