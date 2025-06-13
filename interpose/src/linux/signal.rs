use std::io::{self};

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

use super::client::global_client;
use libc::c_char;
use linux_raw_sys::general as linux_sys;
use std::ffi::CStr;
use arrayvec::ArrayVec;
use fspy_shared::linux::ENVNAME_PROGRAM;
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
                let path_ptr = regs[1] as *const c_char;
                let path = unsafe { CStr::from_ptr(path_ptr) }.to_bytes();

                client
                    .ipc_socket
                    .send_vectored(&[
                        // IoSlice::new( slice::from_ref(&(AccessKind::Open.into()))),
                        IoSlice::new(path),
                    ])
                    .unwrap();
                regs[0] = unsafe {
                    use fspy_shared::linux::SYSCALL_MAGIC;

                    libc::syscall(
                        linux_sys::__NR_openat as _,
                        regs[0],
                        regs[1],
                        regs[2],
                        regs[3],
                        SYSCALL_MAGIC,
                    )
                } as _;
            }
            linux_sys::__NR_execve => {
                let program = regs[0] as *const c_char;
                let argv = regs[1] as *const *const c_char;
                let envp = regs[2] as *const *const c_char;

                let mut program_env_buf =
                    ArrayVec::<u8, { ENVNAME_PROGRAM.len() + 1 + PATH_MAX + 1 }>::new();
                let mut envp_buf = ArrayVec::<*const c_char, 1024>::new();

                // unsafe {
                //     global_state.prepare_envp(program, envp, &mut program_env_buf, &mut envp_buf)
                // };

                // regs[0] = unsafe {
                //     libc::syscall(
                //         linux_sys::__NR_execve as _,
                //         global_state.host_path_env.value().as_ptr(),
                //         argv,
                //         envp_buf.as_ptr(),
                //         SYSCALL_MAGIC,
                //     )
                // } as _;
            }
            _ => {}
        }
    }
}

pub fn install_signal_handler() -> io::Result<()> {
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
