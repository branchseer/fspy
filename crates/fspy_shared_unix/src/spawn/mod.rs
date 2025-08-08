#[cfg(target_os = "linux")]
mod elf;

use fspy_shared::ipc::PathAccess;
use memmap2::Mmap;
use nix::unistd::getcwd;
use seccomp_unotify::payload::SeccompPayload;
use seccomp_unotify::target::install_target;
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::Path;
use std::thread;
use which::which_in;

use crate::exec::{ExecResolveConfig, real_sys_with_callback};
use crate::open_exec::open_executable;
use crate::payload::PAYLOAD_ENV_NAME;

use crate::{
    exec::{Exec, ensure_env},
    payload::EncodedPayload,
};

const LD_PRELOAD: &str = "LD_PRELOAD";

pub struct PreExec(SeccompPayload);
impl PreExec {
    pub fn run(&mut self) -> nix::Result<()> {
        install_target(&self.0)
    }
}

pub fn handle_exec(
    command: &mut Exec,
    config: ExecResolveConfig,
    encoded_payload: &EncodedPayload,
    on_path_access: impl Fn(PathAccess<'_>),
) -> nix::Result<Option<PreExec>> {
    command.resolve(&real_sys_with_callback(on_path_access), config)?;

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
        Ok(Some(PreExec(
            encoded_payload.payload.seccomp_payload.clone(),
        )))
    }
}
