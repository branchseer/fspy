#[cfg(target_os = "linux")]
mod elf;

use bstr::ByteSlice;
use fspy_shared::ipc::{AccessMode, PathAccess};
use memmap2::Mmap;
use nix::unistd::getcwd;
use seccomp_unotify::payload::SeccompPayload;
use seccomp_unotify::target::install_target;
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::Path;
use std::thread;

use crate::exec::ExecResolveConfig;
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
    mut on_path_access: impl FnMut(PathAccess<'_>),
) -> nix::Result<Option<PreExec>> {
    let mut on_path_access = |path_access: PathAccess<'_>| {
        if path_access.path.as_bstr().first() == Some(&b'/') {
            on_path_access(path_access);
        } else {
            let path =
                std::path::absolute(path_access.path.as_os_str()).expect("Failed to get cwd");
            on_path_access(PathAccess {
                path: path.as_path().into(),
                mode: path_access.mode,
            });
        }
    };

    command.resolve(&mut on_path_access, config)?;
    on_path_access(PathAccess {
        mode: AccessMode::Read,
        path: command.program.as_bstr().into(),
    });

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
