use std::{
    ffi::CStr,
    io::{self, IoSlice},
};

use crate::PATH_MAX;
use arrayvec::ArrayVec;
use libc::c_char;
use linux_raw_sys::general as linux_sys;
use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

use crate::{
    GLOBAL_STATE,
    consts::{ENVNAME_PROGRAM, SYSCALL_MAGIC},
    stderr_print, stderr_println,
};

extern "C" fn handle_sigsys(
    _signo: libc::c_int,
    info: *mut libc::siginfo_t,
    data: *mut libc::c_void,
) {
    stderr_print("enter handler...");
    let info = unsafe { info.as_ref().unwrap_unchecked() };
    if info.si_signo != libc::SIGSYS {
        return;
    }

    stderr_println("go");
    // TODO: check why info.si_code isn't SYS_seccomp as documented in seccomp(2)
    // if info.si_code != libc::SYS_seccomp as i32 {
    //     return;
    // }
    let ucontext = unsafe { data.cast::<libc::ucontext_t>().as_mut().unwrap_unchecked() };
    // aarch64
    let regs = &mut ucontext.uc_mcontext.regs;
    let sysno = regs[8] as u32;
    let global_state = unsafe { GLOBAL_STATE.get() };
    match sysno {
        linux_sys::__NR_openat => {
            let path_ptr = regs[1] as *const c_char;
            let path = unsafe { CStr::from_ptr(path_ptr) }.to_bytes();

            stderr_print("path: ");
            stderr_println(path);
            global_state
                .ipc_socket
                .send_vectored(&[
                    // IoSlice::new( slice::from_ref(&(AccessKind::Open.into()))),
                    IoSlice::new(path),
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
            } as _;
        }
        linux_sys::__NR_execve => {
            let program = regs[0] as *const c_char;
            let argv = regs[1] as *const *const c_char;
            let envp = regs[2] as *const *const c_char;

            let mut program_env_buf =
                ArrayVec::<u8, { ENVNAME_PROGRAM.len() + 1 + PATH_MAX + 1 }>::new();
            let mut envp_buf = ArrayVec::<*const c_char, 1024>::new();

            unsafe {
                global_state.prepare_envp(program, envp, &mut program_env_buf, &mut envp_buf)
            };

            regs[0] = unsafe {
                libc::syscall(
                    linux_sys::__NR_execve as _,
                    global_state.host_path_env.value().as_ptr(),
                    argv,
                    envp_buf.as_ptr(),
                    SYSCALL_MAGIC,
                )
            } as _;
        }
        _ => {}
    }
}

pub fn install_siganl_handler() -> io::Result<()> {
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
