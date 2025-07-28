pub mod convert;

use std::{borrow::Cow, cell::RefCell, ffi::CStr, os::fd::RawFd, sync::LazyLock};

use bincode::encode_into_std_write;
use bstr::BStr;
use fspy_shared::{
    ipc::{AccessMode, BINCODE_CONFIG, NativeStr, NativeString, PathAccess},
    linux::{
        PAYLOAD_ENV_NAME, Payload,
        inject::{PayloadWithEncodedString, inject},
    },
    unix::{env::decode_env},
};

use convert::{ToAbsolutePath, ToAccessMode};
use thread_local::ThreadLocal;

pub struct Client {
    payload_with_str: PayloadWithEncodedString,
    tls_shm: ThreadLocal<RefCell<&'static mut [u8]>>,
}

const SHM_CHUNK_SIZE: usize = 65535;

impl Client {
    fn from_env() -> Self {
        let payload_string = std::env::var_os(PAYLOAD_ENV_NAME).unwrap();
        let payload = decode_env::<Payload>(&payload_string);
        Self {
            payload_with_str: PayloadWithEncodedString {
                payload,
                payload_string,
            },
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
