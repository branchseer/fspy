use std::{
    cell::{SyncUnsafeCell, UnsafeCell},
    ffi::{CStr, OsStr, OsString},
    fs::File,
    mem::MaybeUninit,
    os::fd::{FromRawFd as _, RawFd},
};

use allocator_api2::vec::Vec;
use bincode::encode_into_std_write;
use fspy_shared::{
    ipc::{AccessMode, BINCODE_CONFIG, NativeStr, NativeString, PathAccess},
    linux::{
        Payload,
        inject::{PayloadWithEncodedString, inject},
        nul_term::{Env, NulTerminated, find_env},
    },
    unix::cmdinfo::RawCommand,
};
use lexical_core::parse;
use libc::{c_char, c_int};
use nix::sys::socket::MsgFlags;
use socket2::Socket;

use crate::linux::{
    alloc::{StackAllocator, with_stack_allocator},
    path::resolve_path_in,
};

pub struct Client {
    pub program: &'static OsStr,
    pub payload_with_str: PayloadWithEncodedString,
}

impl Client {
    pub unsafe fn handle_exec(
        &self,
        alloc: StackAllocator<'_>,
        raw_command: &mut RawCommand,
    ) -> nix::Result<()> {
        let mut cmd = unsafe { raw_command.into_command(alloc) };
        inject(alloc, &mut cmd, &self.payload_with_str)?;
        *raw_command = RawCommand::from_command(alloc, &cmd);
        Ok(())
    }
    pub unsafe fn handle_open(
        &self,
        dirfd: c_int,
        path: *const c_char,
        flags: c_int,
    ) -> nix::Result<()> {
        let path = unsafe { CStr::from_ptr(path) };
        let acc_mode = match flags & libc::O_ACCMODE {
            libc::O_RDWR => AccessMode::ReadWrite,
            libc::O_WRONLY => AccessMode::Write,
            _ => AccessMode::Read,
        };

        with_stack_allocator(|alloc| {
            let abs_path = resolve_path_in(dirfd, path, alloc)?;
            let path_access = PathAccess {
                mode: acc_mode,
                path: NativeStr::from_bytes(abs_path.to_bytes()),
            };
            let mut msg = Vec::<u8, _>::with_capacity_in(1024, alloc);
            encode_into_std_write(&path_access, &mut msg, BINCODE_CONFIG).unwrap();
            nix::sys::socket::send(
                self.payload_with_str.payload.ipc_fd,
                &msg,
                MsgFlags::empty(),
            )
        })?;
        Ok(())
    }
}

static CLIENT: SyncUnsafeCell<MaybeUninit<Client>> = SyncUnsafeCell::new(MaybeUninit::uninit());

pub unsafe fn init_global_client(client: Client) {
    unsafe { *CLIENT.get() = MaybeUninit::new(client) };
}

pub unsafe fn global_client() -> &'static Client {
    unsafe { (*CLIENT.get()).assume_init_ref() }
}
