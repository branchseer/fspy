use std::{
    io::{self, Write},
    mem::offset_of,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::net::UnixStream as StdUnixStream,
    },
};

use libc::c_int;
use passfd::{tokio::FdPassingExt as _, FdPassingExt as _};
use tokio::{
    io::{unix::AsyncFd, AsyncReadExt},
    net::UnixStream as TokioUnixStream,
    process::Command,
};

fn check_nonnegative(ret: c_int) -> io::Result<()> {
    if ret >= 0 {
        return Ok(());
    }
    Err(io::Error::last_os_error())
}

unsafe fn seccomp(
    operation: libc::c_uint,
    flags: libc::c_uint,
    args: *const libc::c_void,
) -> libc::c_int {
    return c_int::try_from(libc::syscall(libc::SYS_seccomp, operation, flags, args)).unwrap();
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let (mut receiver, sender) = TokioUnixStream::pair()?;
    let sender = sender.into_std()?;
    sender.set_nonblocking(false)?;
    let sender_fd = sender.as_raw_fd();
    let receiver_fd = receiver.as_raw_fd();
    let mut command = Command::new("mkdir");
    command.args(["/usr"]);

    // let x = offset_of!(libc::seccomp_data, arch);

    unsafe {
        command.pre_exec(move || {
            let mut sender = unsafe { StdUnixStream::from_raw_fd(sender_fd) };
            drop(unsafe { StdUnixStream::from_raw_fd(receiver_fd) });
            check_nonnegative(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0))?;

            const X32_SYSCALL_BIT: u32 = 0x40000000;
            let mut filter = [
                libc::BPF_STMT(
                    (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as _,
                    offset_of!(libc::seccomp_data, arch) as _,
                ),
                libc::BPF_JUMP(
                    (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as _,
                    linux_raw_sys::ptrace::AUDIT_ARCH_X86_64,
                    0,
                    2,
                ),
                libc::BPF_STMT(
                    (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as _,
                    offset_of!(libc::seccomp_data, nr) as _,
                ),
                libc::BPF_JUMP(
                    (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as _,
                    X32_SYSCALL_BIT,
                    0,
                    1,
                ),
                libc::BPF_STMT(
                    (libc::BPF_RET | libc::BPF_K) as _,
                    linux_raw_sys::ptrace::SECCOMP_RET_KILL_PROCESS,
                ),
                /* mkdir() triggers notification to user-space supervisor */
                libc::BPF_JUMP(
                    (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as _,
                    libc::SYS_mkdir as _,
                    0,
                    1,
                ),
                libc::BPF_STMT(
                    (libc::BPF_RET + libc::BPF_K) as _,
                    linux_raw_sys::ptrace::SECCOMP_RET_USER_NOTIF,
                ),
                /* Every other system call is allowed */
                libc::BPF_STMT(
                    (libc::BPF_RET | libc::BPF_K) as _,
                    linux_raw_sys::ptrace::SECCOMP_RET_ALLOW,
                ),
            ];

            let prog = libc::sock_fprog {
                len: filter.len() as _,
                filter: filter.as_mut_ptr(),
            };

            let notify_fd = seccomp(
                libc::SECCOMP_SET_MODE_FILTER,
                libc::SECCOMP_FILTER_FLAG_NEW_LISTENER as _,
                ((&prog) as *const libc::sock_fprog).cast(),
            );
            if notify_fd == -1 {
                return Err(io::Error::last_os_error());
            }
            eprintln!("sending");
            sender.send_fd(notify_fd)?;
            sender.flush()?;
            eprintln!("sent");
            Ok(())
        })
    };

    let handle = tokio::spawn(async move {
        let notify_fd = receiver.recv_fd().await?;
        drop(receiver);
        drop(sender);
        let mut async_notify = AsyncFd::new(notify_fd)?;

        const SECCOMP_IOCTL_NOTIF_RECV: libc::c_ulong = 3226476800;
        const SECCOMP_IOCTL_NOTIF_ID_VALID: libc::c_ulong = 1074274562;
        const SECCOMP_IOCTL_NOTIF_SEND: libc::c_ulong = 3222806785;

        let n = libc::seccomp_notif { };

        loop {
            let readable_nofity = async_notify.readable_mut().await?;
            let ret = unsafe { libc::ioctl(*readable_nofity.get_inner(), SECCOMP_USER_NOTIF_FLAG_CONTINUE) };
            
            break;
        }
        io::Result::Ok(())
    });

    let status = command.status().await.unwrap();
    dbg!(status);

    dbg!(handle.await);

    // let mut buf = [0u8; 3];
    // receiver.read_exact(&mut buf).await?;
    // dbg!(std::str::from_utf8(&buf));
    Ok(())
}
