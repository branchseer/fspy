use std::io::{self};

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

use super::SYSCALL_MAGIC;
use super::client::global_client;
use arrayvec::ArrayVec;
use libc::c_char;
use linux_raw_sys::general as linux_sys;
use std::ffi::CStr;

use fspy_shared::unix::cmdinfo::{CommandInfo, RawCommand};

use crate::linux::alloc::with_stack_allocator;

use std::io::IoSlice;
const PATH_MAX: usize = libc::PATH_MAX as usize;

extern "C" fn handle_sigsys(
    _signo: libc::c_int,
    info: *mut libc::siginfo_t,
    data: *mut libc::c_void,
) {
    libc_print::libc_eprintln!("SIGSYS");
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
    #[cfg(target_arch = "aarch64")]
    {
        let regs = &mut ucontext.uc_mcontext.regs;
        let sysno = regs[8] as u32;
        let client = unsafe { global_client() };
        match sysno {
            linux_sys::__NR_openat => {
                use bstr::BStr;
                use libc_print::libc_eprintln;

                let path_ptr = regs[1] as *const c_char;
                let path = unsafe { CStr::from_ptr(path_ptr) }.to_bytes();

                libc_eprintln!("openat {}", BStr::new(path));

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

pub fn install_signal_handler() -> nix::Result<()> {
    // Unset SIGSYS block mask which is preserved across `execve`.
    // See "Signal mask and pending signals" in signal(7)
    let mut sigset = SigSet::empty();
    sigset.add(Signal::SIGSYS);
    sigset.thread_unblock()?;

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
