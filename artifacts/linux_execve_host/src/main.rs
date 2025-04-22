mod bootstrap;
mod consts;
mod nul;
mod exec;
mod hashbang;
mod params;
mod signal;
mod client;

use std::{
    cell::UnsafeCell,
    env::args_os,
    ffi::{CStr, CString, OsStr, c_char},
    fs::File,
    io::Write,
    mem::{ManuallyDrop, MaybeUninit},
    os::{
        fd::{FromRawFd, RawFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    ptr::null,
};

use arrayvec::ArrayVec;
use nul::{Env, NulTerminated, ThinCStr, find_env, iter_environ, iter_envp};
use lexical_core::parse;

use consts::{
    ENVNAME_BOOTSTRAP, ENVNAME_EXECVE_HOST_PATH, ENVNAME_IPC_FD, ENVNAME_PROGRAM,
    ENVNAME_RESERVED_PREFIX,
};

use socket2::Socket;

const PATH_MAX: usize = libc::PATH_MAX as usize;

fn stderr_print(data: impl AsRef<[u8]>) {
    ManuallyDrop::new(unsafe { File::from_raw_fd(libc::STDERR_FILENO) })
        .write_all(data.as_ref())
        .unwrap();
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

fn main() {
    let is_boostrap = unsafe { find_env(ENVNAME_BOOTSTRAP) }.is_some();
    let program_env = unsafe { find_env(ENVNAME_PROGRAM) }.unwrap();
    let host_path_env = unsafe { find_env(ENVNAME_EXECVE_HOST_PATH) }.unwrap();
    let ipc_fd_env = unsafe { find_env(ENVNAME_IPC_FD) }.unwrap();

    let program = OsStr::from_bytes(program_env.value().as_slice());
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

    signal::install_signal_handler().unwrap();

    // eprintln!("before shebang: {} ({})", program.display(), unsafe {
    //     libc::getpid()
    // });

    // let hashbang = {
    //     let program_file = File::open(program).unwrap();
    //     // TODO: check executable permission
    //     let buf_read = BufReader::new(program_file);
    //     parse_hashbang_recursive(
    //         buf_read,
    //         |path| Ok(BufReader::new(File::open(path)?)),
    //         None,
    //         None,
    //     )
    //     .unwrap()
    // };

    // let mut program = Cow::Borrowed(program);
    let mut args: Vec<CString> = vec![];
    for arg in args_os() {
        args.push(CString::new(arg.into_vec()).unwrap());
    }
    // let mut original_args = args_os();
    // if let Some(hashbang) = hashbang {
    //     let _ = original_args.next(); //  Ignoring original argv0. For hashbang scripts, argv0 should be the interpreter.
    //     args = once(hashbang.interpreter.clone())
    //         .chain(hashbang.arguments)
    //         .chain(once(program.into_owned()))
    //         .map(|arg| CString::new(arg.into_vec()).unwrap())
    //         .collect();
    //     program = Cow::Owned(hashbang.interpreter);
    // }


    // eprintln!("after shebang: {} {:?}", program.display(), &args);


    let envs: Vec<&CStr> = unsafe { iter_environ() }
        .flat_map(|data| {
            if is_env_reserved(data) {
                None
            } else {
                Some(data.as_c_str())
            }
        })
        .collect();

    userland_execve::exec(program.as_ref(), &args, &envs)
}
