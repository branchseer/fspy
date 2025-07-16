use std::{
    ffi::{OsString},
    path::Path,
};

use allocator_api2::alloc::Allocator;

use super::Payload;
use crate::unix::cmdinfo::CommandInfo;

pub struct PayloadWithEncodedString {
    pub payload: Payload,
    pub payload_string: OsString,
}

pub fn inject<'a, A: Allocator + Copy + 'a>(
    alloc: A,
    command: &mut CommandInfo<'a, A>,
    payload_with_encoded_str: &'a PayloadWithEncodedString,
) -> nix::Result<()> {
    command.parse_shebang(alloc)?;
    let program = command.program;

    command.program = Path::new(
        payload_with_encoded_str
            .payload
            .execve_host_path
            .as_os_str(),
    );
    // [program] [encoded_payload] [args...]
    command.args.splice(0..0, [ program.as_os_str(), payload_with_encoded_str.payload_string.as_os_str() ]);
    Ok(())
}
