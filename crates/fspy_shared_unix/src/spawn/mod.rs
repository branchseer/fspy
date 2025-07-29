#[cfg(target_os = "linux")]
mod elf;

use std::thread;

use crate::payload::PAYLOAD_ENV_NAME;

use crate::{
    cmdinfo::{CommandInfo, ensure_env},
    payload::EncodedPayload,
};

pub fn handle_spawn<R: Send>(
    mut command: CommandInfo,
    encoded_payload: &EncodedPayload,
    f: impl (FnOnce(CommandInfo) -> nix::Result<R>) + Send,
) -> nix::Result<R> {
    command.parse_shebang()?;
    ensure_env(
        &mut command.envs,
        "LD_PRELOAD",
        encoded_payload.payload.preload_path.as_str(),
    )?;
    ensure_env(
        &mut command.envs,
        PAYLOAD_ENV_NAME,
        &encoded_payload.encoded_string,
    )?;
    thread::scope(|s| s.spawn(|| f(command)).join().unwrap())
}
