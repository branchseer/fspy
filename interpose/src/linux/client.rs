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

use crate::linux::alloc::{StackAllocator, with_stack_allocator};

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
    pub unsafe fn handle_open(&self, dirfd: c_int, path: *const c_char) -> nix::Result<()> {
        let path = unsafe { CStr::from_ptr(path) }.to_bytes();
        with_stack_allocator(|alloc| {
            let path_access = PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_bytes(path),
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
    // let host_path_env = unsafe { find_env(ENVNAME_EXECVE_HOST_PATH) }.unwrap();
    // let ipc_fd_env = unsafe { find_env(ENVNAME_IPC_FD) }.unwrap();
    // let ipc_fd = parse::<RawFd>(ipc_fd_env.value().as_slice()).unwrap();
    // let client = Client {
    //     host_path_env,
    //     ipc_socket: unsafe { Socket::from_raw_fd(ipc_fd) },
    //     ipc_fd_env,
    // };
    unsafe { *CLIENT.get() = MaybeUninit::new(client) };
}

pub unsafe fn global_client() -> &'static Client {
    unsafe { (*CLIENT.get()).assume_init_ref() }
}
