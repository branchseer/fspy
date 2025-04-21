mod consts;
mod params;
mod exec;

use core::slice;
use std::{
    cell::UnsafeCell, env::{self, args_os, current_exe}, ffi::{c_char, c_void, CStr, CString, OsStr}, fs::{File, OpenOptions}, io::{self, Cursor, IoSlice, Write}, mem::{self, MaybeUninit}, os::{fd::{AsFd, AsRawFd, FromRawFd, RawFd}, unix::{ffi::{OsStrExt, OsStringExt}, fs::OpenOptionsExt as _}}, path::Path, ptr::{null, null_mut}, thread::{sleep, spawn}, time::Duration
};

use lexical_core::parse;

use consts::{ENVNAME_HOST_FD, ENVNAME_PROGRAM, ENVNAME_SOCK_FD, SYSCALL_MAGIC};
use libc::{c_int, c_long};
use socket2::Socket;

const PATH_MAX: usize = libc::PATH_MAX as usize;

unsafe fn get_fd_path_if_needed(fd: c_int, path: &CStr, out: &mut [u8; PATH_MAX]) -> io::Result<usize> {
    if unsafe { *path.as_ptr() } == b'/' {
        return Ok(0)
    }
    let mut proc_self_fd_path = [0u8; PATH_MAX];
    core::write!(proc_self_fd_path.as_mut_slice(), "/proc/self/fd/{}\0", fd).unwrap();
    let ret = unsafe  { libc::readlink(proc_self_fd_path.as_ptr(), out.as_mut_ptr(), out.len()) };
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
                unsafe { IPC_SOCKET.get() }.send_vectored(&[
                    // IoSlice::new( slice::from_ref(&(AccessKind::Open.into()))),
                    IoSlice::new(path.to_bytes()),
                ]).unwrap();
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
        Self(UnsafeCell::new( MaybeUninit::uninit() ))
    }
    pub unsafe fn set(&self, value: T) {
        unsafe { self.0.get().write(MaybeUninit::new(value)) }
    }
    pub unsafe fn get(&self) -> &T {
        unsafe { self.0.get().as_ref().unwrap_unchecked().assume_init_ref() }
    }
}
unsafe impl<T> Sync for UnsafeGlobalCell<T> {}

static HOST_FD: UnsafeGlobalCell<RawFd> = UnsafeGlobalCell::uninit();
static IPC_SOCKET: UnsafeGlobalCell<Socket> = UnsafeGlobalCell::uninit();

// const D: &[u8] = include_bytes!("/home/vscode/dbgexe");

fn main() {
    // let memfd = unsafe { libc::memfd_create(c"hello_memfd".as_ptr(), 0) };
    // if memfd < 0 {
    //     return Err(io::Error::last_os_error());
    // }
    // let mut memfd_file = unsafe { File::from_raw_fd(memfd) };
    // memfd_file.write_all(D)?;
    // let memfd = memfd_file.as_fd();
    // let argv: &[*const c_char] = &[c"hello_memfd_argv0".as_ptr(), null()];
    // let envp: &[*const c_char] = &[null()];
    // let ret = unsafe { libc::fexecve(memfd.as_raw_fd(), argv.as_ptr(), envp.as_ptr()) };
    // if ret < 0 {
    //     return Err(io::Error::last_os_error());
    // }
    // Ok(())
    // dbg!(current_exe());
    // let file = OpenOptions::new().read(true).open("/bin/bash").unwrap();
    // unsafe {
    //     let ret = libc::prctl(libc::PR_SET_MM, libc::PR_SET_MM_EXE_FILE, file.as_raw_fd() as c_long, 0 as c_long, 0 as c_long);
    //     if ret == -1 {
    //         Err(io::Error::last_os_error())
    //     } else {
    //         Ok(())
    //     }
    // }.unwrap();
    // dbg!(current_exe());
    // let mut args = argv::iter();
    // let _ = args.next().unwrap(); // argv0

    // let host_mem_fd = parse::<c_int>(args.next().unwrap().as_bytes()).unwrap();
    // let spy_socket_fd = parse::<c_int>(args.next().unwrap().as_bytes()).unwrap();

    // unsafe { HOST_MEM_FD.set(host_mem_fd) };
    // unsafe { SPY_SOCKET_FD.set(Socket::from_raw_fd(spy_socket_fd)) };

    // let program = args.next().unwrap();

    // Allocations could be avoided if we have https://github.com/rust-lang/libs-team/issues/348
    let program = env::var_os(ENVNAME_PROGRAM).unwrap();
    let host_fd = parse::<RawFd>(env::var_os(ENVNAME_HOST_FD).unwrap().as_bytes()).unwrap();
    let socket_fd = parse::<RawFd>(env::var_os(ENVNAME_SOCK_FD).unwrap().as_bytes()).unwrap();
    
    unsafe {
        HOST_FD.set(host_fd);
        IPC_SOCKET.set(Socket::from_raw_fd(socket_fd));
    }

    init_sig_handle().unwrap();

    let args: Vec<CString> = args_os().map(|arg| CString::new(arg.into_vec()).unwrap()).collect();
    let env: Vec<CString> = env::vars_os().filter_map(|(key, value)| {
        if key == ENVNAME_PROGRAM || key == ENVNAME_HOST_FD || key == ENVNAME_HOST_FD {
            return None;
        };
        let mut entry: Vec<u8> = Vec::with_capacity(key.len() + 1 + value.len() + 1);
        entry.extend_from_slice(key.as_bytes());
        entry.push(b'=');
        entry.extend_from_slice(value.as_bytes());
        entry.push(b'\0');
        Some(CString::from_vec_with_nul(entry).unwrap())
    }).collect();
    dbg!(&program);
    userland_execve::exec(Path::new(&program), &args, &env)
}
