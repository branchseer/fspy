#[cfg(target_os = "linux")]
mod elf;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::thread;
use seccomp_unotify::target::install_target;
use memmap2::Mmap;

use crate::open_exec::open_executable;
use crate::payload::PAYLOAD_ENV_NAME;

use crate::{
    cmdinfo::{CommandInfo, ensure_env},
    payload::EncodedPayload,
};

const LD_PRELOAD: &str = "LD_PRELOAD";

pub fn handle_spawn<R: Send>(
    mut command: CommandInfo,
    encoded_payload: &EncodedPayload,
    f: impl (FnOnce(CommandInfo) -> nix::Result<R>) + Send,
) -> nix::Result<R> {
    command.parse_shebang()?;

    let executable_fd = open_executable(Path::new(OsStr::from_bytes(&command.program)))?;
    let executable_mmap = unsafe { Mmap::map(&executable_fd) }
        .map_err(|io_error| nix::Error::try_from(io_error).unwrap_or(nix::Error::UnknownErrno))?;
    if elf::is_dynamically_linked_to_libc(executable_mmap)? {
        ensure_env(
            &mut command.envs,
            LD_PRELOAD,
            encoded_payload.payload.preload_path.as_str(),
        )?;
        ensure_env(
            &mut command.envs,
            PAYLOAD_ENV_NAME,
            &encoded_payload.encoded_string,
        )?;
        f(command)
    } else {
        command
            .envs
            .retain(|(name, _)| name != LD_PRELOAD && name != PAYLOAD_ENV_NAME);
        thread::scope(|s| s.spawn(|| {
            install_target(&encoded_payload.payload.seccomp_payload)?;
            f(command)
        }).join().unwrap())
    }
}
