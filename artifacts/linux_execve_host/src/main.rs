mod bootstrap;
mod consts;
mod exec;
mod params;

use core::slice;
use std::{
    cell::UnsafeCell,
    env::{self, args_os, current_exe},
    ffi::{CStr, CString, OsStr, c_char, c_void},
    fs::{File, OpenOptions},
    io::{self, Cursor, IoSlice, Write},
    mem::{self, MaybeUninit},
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

use lexical_core::parse;

use consts::{
    ENVNAME_BOOTSTRAP, ENVNAME_HOST_FD, ENVNAME_PREFIX, ENVNAME_PROGRAM, ENVNAME_SOCK_FD,
    SYSCALL_MAGIC,
};
use libc::{c_int, c_long};
use null_terminated::Nul;
use socket2::Socket;

const PATH_MAX: usize = libc::PATH_MAX as usize;

unsafe fn get_fd_path_if_needed(
    fd: c_int,
    path: &CStr,
    out: &mut [u8; PATH_MAX],
) -> io::Result<usize> {
    if unsafe { *path.as_ptr() } == b'/' {
        return Ok(0);
    }
    let mut proc_self_fd_path = [0u8; PATH_MAX];
    core::write!(proc_self_fd_path.as_mut_slice(), "/proc/self/fd/{}\0", fd).unwrap();
    let ret = unsafe { libc::readlink(proc_self_fd_path.as_ptr(), out.as_mut_ptr(), out.len()) };
    if let Ok(size) = usize::try_from(ret) {
        Ok(size)
    } else {
        Err(io::Error::last_os_error())
    }
}

fn init_sig_handle() -> io::Result<()> {
    use linux_raw_sys::general as linux_sys;
    unsafe extern "C" fn handle_sigsys(
        _signo: libc::c_int,
        info: *mut libc::siginfo_t,
        data: *mut libc::c_void,
    ) {
        let info = unsafe { info.as_ref().unwrap_unchecked() };
        if info.si_signo != libc::SIGSYS {
            return;
        }
        // TODO: check why info.si_code isn't SYS_seccomp as documented in seccomp(2)
        // if info.si_code != libc::SYS_seccomp as i32 {
        //     return;
        // }
        let ucontext = unsafe { data.cast::<libc::ucontext_t>().as_mut().unwrap_unchecked() };
        // aarch64
        let regs = &mut ucontext.uc_mcontext.regs;
        let sysno = regs[8] as u32;
        match sysno {
            linux_sys::__NR_openat => {
                let path_ptr = regs[1] as *const c_char;
                let path = unsafe { CStr::from_ptr(path_ptr) };
                // libc::write(1, path.as_ptr().cast(), path.to_bytes().len());
                unsafe { &GLOBAL_STATE.get().ipc_socket.fd }
                    .send_vectored(&[
                        // IoSlice::new( slice::from_ref(&(AccessKind::Open.into()))),
                        IoSlice::new(path.to_bytes()),
                    ])
                    .unwrap();
                regs[0] = unsafe {
                    libc::syscall(
                        linux_sys::__NR_openat as _,
                        regs[0],
                        regs[1],
                        regs[2],
                        regs[3],
                        SYSCALL_MAGIC,
                    )
                } as u64;
            }
            linux_sys::__NR_execve => {
                let program = regs[0] as *const c_char;

                // libc::fexecve(fd, argv, envp);
            }
            _ => {}
        }
    }

    let mut sa = unsafe { mem::zeroed::<libc::sigaction>() };
    unsafe {
        libc::sigfillset(&mut sa.sa_mask);
    }
    sa.sa_sigaction = handle_sigsys as *const c_void as usize;
    sa.sa_flags = libc::SA_SIGINFO;

    if unsafe { libc::sigaction(libc::SIGSYS, &sa, null_mut()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
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
unsafe impl<T> Sync for UnsafeGlobalCell<T> {}

struct FdWithStr<FD> {
    fd: FD,
    env: &'static CStr,
}

impl<FD> FdWithStr<FD> {
    fn try_from_env(prefix: &[u8], env: &'static CStr) -> Option<Self>
    where
        FD: From<OwnedFd>,
    {
        let fd_str = env.to_bytes().strip_prefix(prefix)?;
        let fd = parse::<RawFd>(fd_str).unwrap();

        Some(Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) }.into(),
            env,
        })
    }
}

pub fn environ() -> impl Iterator<Item = &'static CStr> {
    unsafe extern "C" {
        static mut environ: *const *const c_char;
    }
    unsafe { envp_to_iter(environ) }
}

unsafe fn envp_to_iter<'a>(envp: *const *const c_char) -> impl Iterator<Item = &'a CStr> {
    let envs = unsafe { Nul::new_unchecked(envp) };
    envs.iter()
        .copied()
        .map(|item| unsafe { CStr::from_ptr(item) })
}

struct GlobalState {
    host_executable: FdWithStr<OwnedFd>,
    ipc_socket: FdWithStr<Socket>,
}
impl GlobalState {
    pub fn prepare_envs<const N: usize>(
        envp: *const *const c_char,
        envp_buf: &mut [*const *const c_char; N],
    ) {
        // let envs = unsafe { envp_to_iter(envp) }
        //     .filter(|item| !item.to_bytes().starts_with(ENVNAME_PREFIX.as_bytes()))
        //     .chain();
    }
}

static GLOBAL_STATE: UnsafeGlobalCell<GlobalState> = UnsafeGlobalCell::uninit();

fn main() {
    // Allocations could be avoided if we have https://github.com/rust-lang/libs-team/issues/348
    if env::var_os(ENVNAME_BOOTSTRAP).is_some() {
        bootstrap::bootstrap();
    }

    let mut program: Option<&[u8]> = None;
    let mut host_executable: Option<FdWithStr<OwnedFd>> = None;
    let mut ipc_socket: Option<FdWithStr<Socket>> = None;
    for env in environ() {
        if let Some(program) = env.to_bytes().strip_prefix(concat!(ENVNAME_PROGRAM,  "=")) {

        }
    }
    let program = env::var_os(ENVNAME_PROGRAM).unwrap();
    let global_state = GlobalState {
        host_executable: FdWithStr::try_from_env(&env::var_os(ENVNAME_HOST_FD).unwrap()),
        ipc_socket: FdWithStr::try_from_env(&env::var_os(ENVNAME_SOCK_FD).unwrap()),
    };

    unsafe {
        GLOBAL_STATE.set(global_state);
    }

    init_sig_handle().unwrap();

    let args: Vec<CString> = args_os()
        .map(|arg| CString::new(arg.into_vec()).unwrap())
        .collect();
    let env: Vec<&'static CStr> = environ()
        .filter_map(|item| {
            if item.to_bytes().starts_with(ENVNAME_PREFIX.as_bytes()) {
                return None;
            }
            Some(item)
        })
        .collect();
    userland_execve::exec(Path::new(&program), &args, &env)
}
