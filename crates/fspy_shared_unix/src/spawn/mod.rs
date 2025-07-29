#[cfg(target_os = "linux")]
mod elf;

use memmap2::Mmap;
use nix::unistd::getcwd;
use seccomp_unotify::payload::SeccompPayload;
use seccomp_unotify::target::install_target;
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::Path;
use std::thread;
use which::which_in;

use crate::open_exec::open_executable;
use crate::payload::PAYLOAD_ENV_NAME;

use crate::{
    cmdinfo::{CommandInfo, ensure_env},
    payload::EncodedPayload,
};

const LD_PRELOAD: &str = "LD_PRELOAD";

pub struct PreSpawn(SeccompPayload);
impl PreSpawn {
    pub fn run(&mut self) -> nix::Result<()> {
        install_target(&self.0)
    }
}

pub fn handle_spawn<'a>(
    command: &mut CommandInfo,
    find_in_path: bool,
    encoded_payload: &'a EncodedPayload,
) -> nix::Result<Option<PreSpawn>> {
    if find_in_path {
        let path = command.envs.iter().find_map(|(name, value)| {
            if name.eq_ignore_ascii_case(b"PATH") {
                let value = value.as_ref()?;
                Some(OsStr::from_bytes(value))
            } else {
                None
            }
        });
        let cwd = getcwd()?;
        let program = which_in(OsStr::from_bytes(&command.program), path, cwd)
            .map_err(|_| nix::Error::ENOENT)?;
        command.program = program.into_os_string().into_vec().into();
    }
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
        Ok(None)
    } else {
        command
            .envs
            .retain(|(name, _)| name != LD_PRELOAD && name != PAYLOAD_ENV_NAME);
        Ok(Some(PreSpawn(encoded_payload.payload.seccomp_payload.clone())))
    }
}
