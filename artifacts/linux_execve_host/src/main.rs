mod bootstrap;
mod consts;
mod env;
mod exec;
mod params;
mod signal;

use core::slice;
use std::{
    cell::UnsafeCell,
    env::{args_os, current_exe},
    ffi::{CStr, CString, OsStr, c_char, c_void},
    fs::{File, OpenOptions},
    io::{self, Cursor, IoSlice, Write},
    mem::{self, ManuallyDrop, MaybeUninit},
    os::{
        fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::OpenOptionsExt as _,
        },
    },
    path::Path,
    ptr::{null, null_mut},
    thread::{sleep, spawn},
    time::Duration,
};

use arrayvec::ArrayVec;
use env::{Env, Terminated, ThinCStr, find_env, iter_environ, iter_envp};
use lexical_core::parse;

use consts::{
    ENVNAME_BOOTSTRAP, ENVNAME_EXECVE_HOST_PATH, ENVNAME_IPC_FD, ENVNAME_PROGRAM,
    ENVNAME_RESERVED_PREFIX, SYSCALL_MAGIC,
};
use libc::{c_int, c_long};
use socket2::Socket;

const PATH_MAX: usize = libc::PATH_MAX as usize;

fn stderr_print(data: impl AsRef<[u8]>) {
    ManuallyDrop::new(unsafe { File::from_raw_fd(libc::STDERR_FILENO) }).write_all(data.as_ref());
}
fn stderr_println(data: impl AsRef<[u8]>) {
    stderr_print(data);
    stderr_print(b"\n");
}

// unsafe fn get_fd_path_if_needed(
//     fd: c_int,
//     path: &CStr,
//     out: &mut [u8; PATH_MAX],
// ) -> io::Result<usize> {
//     if unsafe { *path.as_ptr() } == b'/' {
//         return Ok(0);
//     }
//     let mut proc_self_fd_path = [0u8; PATH_MAX];
//     core::write!(proc_self_fd_path.as_mut_slice(), "/proc/self/fd/{}\0", fd).unwrap();
//     let ret = unsafe { libc::readlink(proc_self_fd_path.as_ptr(), out.as_mut_ptr(), out.len()) };
//     if let Ok(size) = usize::try_from(ret) {
//         Ok(size)
//     } else {
//         Err(io::Error::last_os_error())
//     }
// }


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
unsafe impl<T> Sync for UnsafeGlobalCell<T> {}

struct GlobalState {
    host_path_env: Env<'static>,
    ipc_socket: Socket,
    ipc_fd_env: Env<'static>,
}

fn is_env_reserved(env: ThinCStr<'_>) -> bool {
    let mut iter = env.iter().copied();
    for ch in ENVNAME_RESERVED_PREFIX.as_bytes() {
        if iter.next() != Some(*ch) {
            return false;
        }
    }
    true
}

impl GlobalState {
    unsafe fn prepare_envp<const N: usize, const M: usize>(
        &self,
        program: *const c_char,
        envp: *const *const c_char,
        program_env_buf: &mut ArrayVec<u8, N>,
        envp_buf: &mut ArrayVec<*const c_char, M>,
    ) {
        let program = unsafe { Terminated::new_unchecked(program) }.to_fat();
        program_env_buf.clear();
        program_env_buf
            .try_extend_from_slice(ENVNAME_PROGRAM.as_bytes())
            .unwrap();
        program_env_buf.push(b'=');
        program_env_buf
            .try_extend_from_slice(program.as_slice_with_term())
            .unwrap();

        envp_buf.clear();
        envp_buf.push(program_env_buf.as_ptr());
        envp_buf.push(self.host_path_env.data().as_ptr());
        envp_buf.push(self.ipc_fd_env.data().as_ptr());

        for env in unsafe { iter_envp(envp) } {
            if is_env_reserved(env) {
                let env_data = env.to_fat().as_slice();

                stderr_print(b"fspy: child process should not spawn with reserved env name (");
                stderr_print(env_data);
                stderr_print(b")\n");
                unsafe { libc::abort() };
            }
            envp_buf.push(env.as_ptr());
        }
        envp_buf.push(null());
    }
}

static GLOBAL_STATE: UnsafeGlobalCell<GlobalState> = UnsafeGlobalCell::uninit();

fn main() {
    let is_boostrap = unsafe { find_env(ENVNAME_BOOTSTRAP) }.is_some();
    let program_env = unsafe { find_env(ENVNAME_PROGRAM) }.unwrap();
    let host_path_env = unsafe { find_env(ENVNAME_EXECVE_HOST_PATH) }.unwrap();
    let ipc_fd_env = unsafe { find_env(ENVNAME_IPC_FD) }.unwrap();

    let program = Path::new(OsStr::from_bytes(program_env.value().as_slice()));
    let ipc_fd = parse::<RawFd>(ipc_fd_env.value().as_slice()).unwrap();
    let global_state = GlobalState {
        host_path_env,
        ipc_socket: unsafe { Socket::from_raw_fd(ipc_fd) },
        ipc_fd_env,
    };

    unsafe {
        GLOBAL_STATE.set(global_state);
    }

    if is_boostrap {
        bootstrap::bootstrap().unwrap();
    }

    signal::install_siganl_handler().unwrap();

    let args: Vec<CString> = args_os()
        .map(|arg| CString::new(arg.into_vec()).unwrap())
        .collect();
    let envs: Vec<&CStr> = unsafe { iter_environ() }
        .flat_map(|data| {
            if is_env_reserved(data) {
                None
            } else {
                Some(data.as_c_str())
            }
        })
        .collect();

    println!("pid: {}", unsafe { libc::getpid() });
    stderr_print("program: ");
    stderr_println(program.as_os_str().as_bytes());
    dbg!(unsafe { libc::open(c"/".as_ptr(), 0) });
    userland_execve::exec(program, &args, &envs)
}
