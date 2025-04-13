use std::{
    ffi::c_void,
    io::{self, Write},
    mem::{offset_of, zeroed},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::net::UnixStream as StdUnixStream,
    },
};

use libc::{c_int, memset, SECCOMP_GET_NOTIF_SIZES};
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
    args: *mut libc::c_void,
) -> libc::c_int {
    return c_int::try_from(libc::syscall(libc::SYS_seccomp, operation, flags, args)).unwrap();
}

struct SeccompNotifyBuf {
    req: *mut c_void,
    req_size: usize,
    resp: *mut c_void,
    resp_size: usize,
}
unsafe impl Send for SeccompNotifyBuf {}
impl SeccompNotifyBuf {
    pub fn alloc() -> io::Result<Self> {
        let mut sizes = unsafe { zeroed::<libc::seccomp_notif_sizes>() };
        check_nonnegative(unsafe { seccomp(SECCOMP_GET_NOTIF_SIZES, 0, (&raw mut sizes).cast()) })?;
        let req_size = sizes.seccomp_notif as usize;
        // TODO: use global allocator (make sure the alignment is correct)
        let req = unsafe { libc::malloc(req_size) };
        if req.is_null() {
            return Err(io::Error::last_os_error());
        }
        // check the example in https://man7.org/linux/man-pages/man2/seccomp_unotify.2.html
        let resp_size = libc::size_t::max(
            size_of::<libc::seccomp_notif_resp>(),
            sizes.seccomp_notif_resp as _,
        );
        let resp = unsafe { libc::malloc(resp_size) };
        if resp.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            req,
            req_size,
            resp,
            resp_size,
        })
    }
    pub fn zeroed(&mut self) -> (&mut libc::seccomp_notif, &mut libc::seccomp_notif_resp) {
        unsafe { self.req.write_bytes(0, self.req_size) };
        unsafe { self.resp.write_bytes(0, self.resp_size) };
        unsafe {
            (
                self.req
                    .cast::<libc::seccomp_notif>()
                    .as_mut()
                    .unwrap_unchecked(),
                self.resp
                    .cast::<libc::seccomp_notif_resp>()
                    .as_mut()
                    .unwrap_unchecked(),
            )
        }
    }
}
impl Drop for SeccompNotifyBuf {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.req);
            libc::free(self.resp)
        }
    }
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

            let mut prog = libc::sock_fprog {
                len: filter.len() as _,
                filter: filter.as_mut_ptr(),
            };

            let notify_fd = seccomp(
                libc::SECCOMP_SET_MODE_FILTER,
                libc::SECCOMP_FILTER_FLAG_NEW_LISTENER as _,
                (&raw mut prog).cast(),
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

        let mut notify_buf = SeccompNotifyBuf::alloc()?;

        loop {
            let (req, resp) = notify_buf.zeroed();
            let readable_nofity = async_notify.readable_mut().await?;
            let ret = unsafe { libc::ioctl(*readable_nofity.get_inner(), SECCOMP_IOCTL_NOTIF_RECV, &raw mut *req) };
            if ret < 0 && unsafe { *libc::__errno_location() } == libc::EINTR {
                continue;
            }
            check_nonnegative(ret)?;

            dbg!((req.data.nr, libc::SYS_mkdir));

            resp.id = req.id;
            resp.flags = libc::SECCOMP_USER_NOTIF_FLAG_CONTINUE as _;
            let writable_notify = async_notify.writable_mut().await?;
            let ret = unsafe { libc::ioctl(*writable_notify.get_inner(), SECCOMP_IOCTL_NOTIF_SEND, &raw mut *resp) };
            if ret < 0 && unsafe { *libc::__errno_location() } == libc::ENOENT {
                continue;
            } 
            check_nonnegative(ret)?;

            eprintln!("resp sent");

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
