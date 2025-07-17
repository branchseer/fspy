use std::io::{self};

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

use super::SYSCALL_MAGIC;
use super::client::global_client;
use arrayvec::ArrayVec;
use libc::c_char;
use linux_raw_sys::general as linux_sys;
use std::ffi::CStr;

use std::os::fd::RawFd;
use std::os::raw::c_int;

use std::os::unix::ffi::OsStrExt as _;

use bstr::BStr;
use libc_print::libc_eprintln;
use std::cmp::min;

use fspy_shared::unix::cmdinfo::{CommandInfo, RawCommand};

use crate::linux::{abort::abort_with, alloc::with_stack_allocator};

use std::io::IoSlice;
const PATH_MAX: usize = libc::PATH_MAX as usize;

extern "C" fn handle_sigsys(
    _signo: libc::c_int,
    info: *mut libc::siginfo_t,
    data: *mut libc::c_void,
) {
    let info = unsafe { info.as_ref().unwrap_unchecked() };
    if info.si_signo != libc::SIGSYS {
        return;
    }

    if let Err(err) = unblock_sigsys() {
        abort_with("failed to unblock SIGSYS")
    }

    // TODO: check why info.si_code isn't SYS_seccomp as documented in seccomp(2)
    // if info.si_code != libc::SYS_seccomp as i32 {
    //     return;
    // }
    let ucontext = unsafe { data.cast::<libc::ucontext_t>().as_mut().unwrap_unchecked() };
    // aarch64
    #[cfg(target_arch = "aarch64")]
    {
        let regs = &mut ucontext.uc_mcontext.regs;
        let sysno = regs[8] as u32;
        let client = unsafe { global_client() };
        match sysno {
            linux_sys::__NR_readlinkat => {
                // See "EXAMPLES" section in https://man7.org/linux/man-pages/man2/memfd_create.2.html
                const EXECVE_HOST_PATH_PREFIX: &str =
                    const_format::concatcp!("/memfd:", fspy_shared::linux::EXECVE_HOST_NAME);

                let dir_fd = regs[0] as RawFd;
                let path = regs[1] as *const c_char;
                let orig_buf = regs[2] as *mut c_char;
                let orig_bufsiz = regs[3] as c_int;

                // make sure buf is large enough to read the whole EXECVE_HOST_PATH_PREFIX
                let mut fit_buf = [0u8; EXECVE_HOST_PATH_PREFIX.len()];
                let fit_bufsize = fit_buf.len() as c_int;
                let (buf, bufsiz) = if orig_bufsiz < fit_bufsize {
                    (fit_buf.as_mut_ptr(), fit_bufsize)
                } else {
                    (orig_buf, orig_bufsiz)
                };

                let ret = unsafe {
                    libc::syscall(
                        linux_sys::__NR_readlinkat as _,
                        dir_fd,
                        path,
                        buf,
                        bufsiz,
                        super::SYSCALL_MAGIC,
                    )
                };
                regs[0] = ret as _;

                let Ok(out_len) = usize::try_from(ret) else {
                    // error
                    return;
                };

                let out = unsafe { core::slice::from_raw_parts(buf, out_len) };

                let real_out = if out.starts_with(EXECVE_HOST_PATH_PREFIX.as_bytes()) {
                    client.program.as_bytes()
                } else {
                    out
                };

                if real_out.as_ptr() != orig_buf {
                    let len = min(real_out.len(), orig_bufsiz as usize);
                    unsafe { orig_buf.copy_from_nonoverlapping(real_out.as_ptr(), len) };
                    regs[0] = len as _;
                }
            }
            linux_sys::__NR_openat => {
                let dir_fd = regs[0] as RawFd;
                let path_ptr = regs[1] as *const c_char;
                let path = unsafe { CStr::from_ptr(path_ptr) }.to_bytes();

                libc_eprintln!("openat {} {} {}", dir_fd, libc::AT_FDCWD, BStr::new(path));

                regs[0] = unsafe {
                    libc::syscall(
                        linux_sys::__NR_openat as _,
                        regs[0],
                        regs[1],
                        regs[2],
                        regs[3],
                        super::SYSCALL_MAGIC,
                    )
                } as _;
            }
            linux_sys::__NR_execve => {
                let mut raw_command = RawCommand {
                    prog: regs[0] as *const c_char,
                    argv: regs[1] as *const *const c_char,
                    envp: regs[2] as *const *const c_char,
                };
                let result: nix::Result<i64> = with_stack_allocator(|alloc| {
                    libc_print::libc_eprintln!("execve with alloc");
                    unsafe { client.handle_exec(alloc, &mut raw_command) }?;
                    Ok(unsafe {
                        libc::syscall(
                            linux_sys::__NR_execve as _,
                            raw_command.prog,
                            raw_command.argv,
                            raw_command.envp,
                            SYSCALL_MAGIC,
                        )
                    })
                });
                if let Err(err) = result {
                    regs[0] = (-(err as i64)) as u64
                }
            }
            _ => {}
        }
    }
}

fn unblock_sigsys() -> nix::Result<()> {
    let mut sigset = SigSet::empty();
    sigset.add(Signal::SIGSYS);
    sigset.thread_unblock()
}

pub fn install_signal_handler() -> nix::Result<()> {
    // Unset SIGSYS block mask which is preserved across `execve`.
    // See "Signal mask and pending signals" in signal(7)
    unblock_sigsys()?;

    unsafe {
        sigaction(
            Signal::SIGSYS,
            &SigAction::new(
                SigHandler::SigAction(handle_sigsys),
                SaFlags::SA_RESTART,
                SigSet::all(),
            ),
        )
    }?;
    Ok(())
}
