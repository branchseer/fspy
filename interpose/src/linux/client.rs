use std::{cell::{SyncUnsafeCell, UnsafeCell}, ffi::OsString, fs::File, mem::MaybeUninit, os::fd::{FromRawFd as _, RawFd}};

use fspy_shared::{ipc::NativeString, linux::{inject::{inject, PayloadWithEncodedString}, nul_term::{find_env, Env, NulTerminated}, Payload}, unix::cmdinfo::RawCommand};
use lexical_core::parse;
use socket2::Socket;

use crate::linux::alloc::StackAllocator;

pub struct Client {
    pub payload_with_str: PayloadWithEncodedString,
}

impl Client {
    pub unsafe fn handle_exec(&self, alloc: StackAllocator<'_>, raw_command: &mut RawCommand) -> nix::Result<()> {
        let mut cmd = unsafe { raw_command.into_command(alloc) };
        libc_print::libc_eprintln!("before inject {:?}", &cmd);
        inject(alloc, &mut cmd, &self.payload_with_str)?;
        libc_print::libc_eprintln!("after inject {:?}", &cmd);
        *raw_command = RawCommand::from_command(alloc, &cmd);
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
