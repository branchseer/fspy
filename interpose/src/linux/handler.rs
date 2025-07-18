use std::io::{self};
use std::mem::transmute_copy;
use std::ptr::{null, null_mut};

use nix::errno::Errno;
use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal};

use super::SYSCALL_MAGIC;
use super::client::global_client;
use arrayvec::ArrayVec;
use libc::{c_char, c_ulong};
use std::ffi::{CStr, c_uint};
use syscalls::{Sysno, raw_syscall};

use libc::size_t;

use libc::sigaction;

use std::os::fd::RawFd;
use std::os::raw::{c_int, c_void};

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

        if sysno == (Sysno::rt_sigaction as _) {
            let signum = regs[0] as c_int;
            let mut act = regs[1] as *const sigaction;
            let oldact = regs[2] as *mut sigaction;
            let size = regs[3] as size_t;

            if signum == Signal::SIGSYS as _ {
                // Ignore application SIGSYS handler
                // TODO: manage application SIGSYS handler inside this handler
                act = null()
            }

            regs[0] = unsafe {
                raw_syscall!(
                    Sysno::rt_sigaction,
                    signum,
                    act,
                    oldact,
                    size,
                    super::SYSCALL_MAGIC
                )
            } as _;
        } else if sysno == (Sysno::readlinkat as _) {
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

            let out_len = unsafe {
                raw_syscall!(
                    Sysno::readlinkat as _,
                    dir_fd,
                    path,
                    buf,
                    bufsiz,
                    super::SYSCALL_MAGIC
                )
            };
            regs[0] = out_len as _;

            if ((out_len as isize) < 0) {
                return;
            }

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
        } else if sysno == (Sysno::openat as _) {
            let dirfd = regs[0] as RawFd;
            let path_ptr = regs[1] as *const c_char;
            let flags = regs[2] as c_int;
            let mode = regs[3] as libc::mode_t;

            let nix_result = unsafe { client.handle_open(dirfd, path_ptr) };
            if let Err(err) = nix_result {
                regs[0] = (-(err as i64)) as u64;
                return;
            } 

            regs[0] = unsafe {
                raw_syscall!(
                    Sysno::openat,
                    dirfd,
                    path_ptr,
                    flags,
                    mode,
                    super::SYSCALL_MAGIC
                )
            } as _;
        } else if sysno == (Sysno::execve as _) {
            let mut raw_command = RawCommand {
                prog: regs[0] as *const c_char,
                argv: regs[1] as *const *const c_char,
                envp: regs[2] as *const *const c_char,
            };
            let result: nix::Result<()> = with_stack_allocator(|alloc| {
                // libc_print::libc_eprintln!("execve with alloc");
                unsafe { client.handle_exec(alloc, &mut raw_command) }
            });
            if let Err(err) = result {
                regs[0] = (-(err as i64)) as u64;
                return;
            } 
            regs[0] = unsafe {
                raw_syscall!(
                    Sysno::execve,
                    raw_command.prog,
                    raw_command.argv,
                    raw_command.envp,
                    SYSCALL_MAGIC
                )
            } as _;
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
        let new_sigact = SigAction::new(
            SigHandler::SigAction(handle_sigsys),
            SaFlags::SA_RESTART,
            SigSet::all(),
        );
        // nix::sys::signal::sigaction(Signal::SIGSYS, &new_act)?;
        let libc_new_sigact = libc::sigaction::from(new_sigact);
        // https://github.com/kraj/musl/blob/1b06420abdf46f7d06ab4067e7c51b8b63731852/src/internal/ksigaction.h#L1
        // https://github.com/kraj/musl/blob/1b06420abdf46f7d06ab4067e7c51b8b63731852/src/signal/sigaction.c#L47
        #[allow(non_camel_case_types)]
        #[repr(C)]
        struct k_sigaction {
            handler: usize,
            flags: c_ulong,
            restorer: *const c_void,
            mask: [c_uint; 2],
        }
        // const SA_RESTORER: c_ulong = 0x04000000;
        // unsafe extern "C" {
        //     unsafe fn __restore_rt();
        // }
        let kernel_new_sigact = k_sigaction {
            handler: libc_new_sigact.sa_sigaction,
            flags: (libc_new_sigact.sa_flags as c_ulong),
            restorer: null(),
            mask: transmute_copy(&libc_new_sigact.sa_mask),
        };
        let ret = raw_syscall!(
            Sysno::rt_sigaction as _,
            Signal::SIGSYS as c_int,
            &kernel_new_sigact as *const k_sigaction,
            null_mut::<k_sigaction>(),
            8 as size_t,
            super::SYSCALL_MAGIC
        ) as isize;
        if ret < 0 {
            return Err(Errno::from_raw((-ret as i32)));
        }
    };
    Ok(())
}
