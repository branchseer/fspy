mod consts;

use std::{
    cell::UnsafeCell, env::args_os, ffi::{c_char, c_void, CStr}, io::{self, IoSlice}, mem::{self, MaybeUninit}, os::{fd::FromRawFd, unix::ffi::OsStrExt}, ptr::{null, null_mut}, thread::{sleep, spawn}, time::Duration
};

use lexical_core::parse;

use consts::SYSCALL_MAGIC;
use libc::{c_int};
use socket2::Socket;

fn init_sig_handle() -> io::Result<()> {
    use linux_raw_sys::general as linux_sys;
    unsafe extern "C" fn handle_sigsys(
        _signo: libc::c_int,
        info: *mut libc::siginfo_t,
        data: *mut libc::c_void,
    ) {
        let info = unsafe { info.as_ref().unwrap_unchecked() };
        if info.si_code != libc::SYS_seccomp as i32 {
            return;
        }
        let ucontext = unsafe { data.cast::<libc::ucontext_t>().as_mut().unwrap_unchecked() };
        let regs = &mut ucontext.uc_mcontext.regs;
        // aarch64
        let sysno = regs[8] as u32;
        match sysno {
            linux_sys::__NR_openat => {
                let fd = unsafe {
                    libc::syscall(
                        linux_sys::__NR_openat as _,
                        regs[0],
                        regs[1],
                        regs[2],
                        regs[3],
                        SYSCALL_MAGIC,
                    )
                };
                let path_ptr = regs[1] as *const c_char;
                let path = unsafe { CStr::from_ptr(path_ptr) };
                let bufs = IoSlice::new(&[]);
                SPY_SOCKET_FD.get_ref().send_vectored(bufs);
                regs[0] = fd as u64;
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
    pub unsafe fn get(&self) -> T where T: Copy {
        unsafe { self.0.get().read().assume_init() }
    }
    pub unsafe fn get_ref(&self) -> &T {
        unsafe { self.0.get().as_ref().unwrap_unchecked().assume_init_ref() }
    }
}
unsafe impl<T> Sync for UnsafeGlobalCell<T> {}

static HOST_MEM_FD: UnsafeGlobalCell<libc::c_int> = UnsafeGlobalCell::uninit();
static SPY_SOCKET_FD: UnsafeGlobalCell<Socket> = UnsafeGlobalCell::uninit();

fn main() -> ! {
    let mut args = argv::iter();
    let _ = args.next().unwrap(); // argv0

    let host_mem_fd = parse::<c_int>(args.next().unwrap().as_bytes()).unwrap();
    let spy_socket_fd = parse::<c_int>(args.next().unwrap().as_bytes()).unwrap();

    unsafe { HOST_MEM_FD.set(host_mem_fd) };
    unsafe { SPY_SOCKET_FD.set(Socket::from_raw_fd(spy_socket_fd)) };

    let program = args.next().unwrap();
    init_sig_handle().unwrap();

    let args: &[&CStr] = &[];
    let env: &[&CStr] = &[];
    userland_execve::exec("/bin/bash".as_ref(), args, env)
}
