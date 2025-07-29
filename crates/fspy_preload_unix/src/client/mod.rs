pub mod convert;
pub mod raw_cmd;

use std::{borrow::Cow, cell::RefCell, ffi::CStr, os::fd::RawFd, sync::LazyLock};

use bincode::encode_into_std_write;
use bstr::BStr;
use fspy_shared::ipc::{AccessMode, BINCODE_CONFIG, NativeStr, NativeString, PathAccess};
use fspy_shared_unix::{
    payload::{EncodedPayload, decode_payload_from_env},
    spawn::{PreSpawn, handle_spawn},
};

use convert::{ToAbsolutePath, ToAccessMode};
use raw_cmd::RawCommand;
use thread_local::ThreadLocal;

pub struct Client {
    encoded_payload: EncodedPayload,
    tls_shm: ThreadLocal<RefCell<&'static mut [u8]>>,
}

const SHM_CHUNK_SIZE: usize = 65535;

impl Client {
    fn from_env() -> Self {
        let encoded_payload = decode_payload_from_env().unwrap();
        Self {
            encoded_payload,
            tls_shm: ThreadLocal::new(),
        }
    }
    // pub unsafe fn handle_exec(
    //     &self,
    //     alloc: StackAllocator<'_>,
    //     raw_command: &mut RawCommand,
    // ) -> nix::Result<()> {
    //     let mut cmd = unsafe { raw_command.into_command(alloc) };
    //     inject(alloc, &mut cmd, &self.payload_with_str)?;
    //     *raw_command = RawCommand::from_command(alloc, &cmd);
    //     Ok(())
    // }

    fn send(&self, path_access: PathAccess<'_>) -> nix::Result<()> {
        let buf = self
            .tls_shm
            .get_or_try(|| nix::Result::Ok(RefCell::new(&mut [])))?;
        dbg!(path_access);
        Ok(())
    }

    pub unsafe fn handle_spawn<R>(
        &self,
        find_in_path: bool,
        raw_command: RawCommand,
        f: impl FnOnce(RawCommand, Option<PreSpawn>) -> nix::Result<R>,
    ) -> nix::Result<R> {
        let mut cmd_info = unsafe { raw_command.into_command() };
        let pre_spawn = handle_spawn(&mut cmd_info, find_in_path, &self.encoded_payload)?;
        RawCommand::from_command(cmd_info, |raw_command| f(raw_command, pre_spawn))
    }

    pub unsafe fn handle_open(
        &self,
        path: impl ToAbsolutePath,
        mode: impl ToAccessMode,
    ) -> nix::Result<()> {
        let mode = unsafe { mode.to_access_mode() };
        let () = unsafe {
            path.to_absolute_path(|abs_path| {
                self.send(PathAccess {
                    mode,
                    path: abs_path.into(),
                })
            })
        }?;

        Ok(())
    }
}

pub unsafe fn global_client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| Client::from_env());
    &CLIENT
}
