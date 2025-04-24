use std::{cell::UnsafeCell, fs::File, mem::MaybeUninit, os::fd::{FromRawFd as _, RawFd}};

use lexical_core::parse;
use socket2::Socket;

use crate::{consts::{ENVNAME_EXECVE_HOST_PATH, ENVNAME_IPC_FD}, nul::{find_env, Env, NulTerminated}};

pub struct Client<'a> {
    pub host_path_env: Env<'a>,
    pub ipc_socket: Socket,
    pub ipc_fd_env: Env<'a>,
}

impl<'a> Client<'a> {
  pub fn open(path: NulTerminated<'_, u8>) -> nix::Result<File> {
    todo!()
  }
}


struct UnsafeGlobalCell<T>(UnsafeCell<MaybeUninit<T>>);
impl<T> UnsafeGlobalCell<T> {
    pub const fn uninit() -> Self {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }
    pub unsafe fn set(&self, value: T) {
        unsafe { self.0.get().write(MaybeUninit::new(value)) }
    }
    pub unsafe fn get(&self) -> &T {
        unsafe { self.0.get().as_ref().unwrap_unchecked().assume_init_ref() }
    }
}
unsafe impl<T: Sync> Sync for UnsafeGlobalCell<T> {}


static CLIENT: UnsafeGlobalCell<Client<'static>> = UnsafeGlobalCell::uninit();

pub unsafe fn init_global_client() {
    let host_path_env = unsafe { find_env(ENVNAME_EXECVE_HOST_PATH) }.unwrap();
    let ipc_fd_env = unsafe { find_env(ENVNAME_IPC_FD) }.unwrap();
    let ipc_fd = parse::<RawFd>(ipc_fd_env.value().as_slice()).unwrap();
    let client = Client {
        host_path_env,
        ipc_socket: unsafe { Socket::from_raw_fd(ipc_fd) },
        ipc_fd_env,
    };
    unsafe { CLIENT.set(client) };
}

pub unsafe fn global_client() -> &'static Client<'static> {
    unsafe { CLIENT.get() }
}
