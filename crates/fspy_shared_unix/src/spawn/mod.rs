#[cfg(target_os = "linux")]
mod elf;

use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _, thread};

use crate::payload::PAYLOAD_ENV_NAME;
use allocator_api2::alloc::Allocator;

use crate::{
    cmdinfo::{CommandInfo, CommandInfoRef, ensure_env},
    payload::EncodedPayload,
};

pub fn handle_spawn<'a, A: Allocator + Copy + 'a, R: Send>(
    alloc: A,
    mut command: CommandInfo<'a, A>,
    encoded_payload: &'a EncodedPayload,
    f: impl (FnOnce(CommandInfoRef<'_>) -> nix::Result<R>) + Send,
) -> nix::Result<R> {
    command.parse_shebang(alloc)?;
    ensure_env(
        &mut command.envs,
        OsStr::from_bytes(b"LD_PRELOAD"),
        encoded_payload.payload.preload_path.as_str().as_ref(),
    )?;
    ensure_env(
        &mut command.envs,
        OsStr::from_bytes(PAYLOAD_ENV_NAME.as_bytes()),
        &encoded_payload.encoded_string,
    )?;
    let cmd_info_ref = command.as_cmd_info_ref();
    thread::scope(|s| s.spawn(|| f(cmd_info_ref)).join().unwrap())
}
