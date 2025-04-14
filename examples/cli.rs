use std::{
    collections::HashSet, convert::Infallible, env::args, ffi::{c_void, CStr}, future::pending, io::{self, Write}, mem::{offset_of, zeroed}, os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{net::UnixStream, thread::JoinHandleExt},
    }, pin::pin, str::from_utf8, thread::{park, sleep, spawn, Thread}, time::Duration
};

use futures_util::future::{select, Either};
use libc::{c_int, memset, SECCOMP_GET_NOTIF_SIZES};
use passfd::FdPassingExt as _;
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, TargetArch};
use tokio::{
    io::{unix::AsyncFd, AsyncReadExt},
    process::Command,
    sync::mpsc,
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

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        fn current_target_arch() -> TargetArch { TargetArch::x86_64 }
    } else if #[cfg(target_arch = "aarch64")] {
        fn current_target_arch() -> TargetArch { TargetArch::aarch64 }
    } else {
        fn current_target_arch() -> TargetArch { compile_error!("Unsupported arch") }
    }
}

unsafe fn collect_paths(notify_fd: libc::c_int, mut on_path: impl FnMut(&[u8])) -> io::Result<()> {
    const SECCOMP_IOCTL_NOTIF_RECV: libc::c_ulong = 3226476800;
    const SECCOMP_IOCTL_NOTIF_ID_VALID: libc::c_ulong = 1074274562;
    const SECCOMP_IOCTL_NOTIF_SEND: libc::c_ulong = 3222806785;

    let mut notify_buf = SeccompNotifyBuf::alloc()?;

    let mut path_buf = [0u8; libc::PATH_MAX as usize];

    loop {
        let (req, resp) = notify_buf.zeroed();
        let ret = unsafe { libc::ioctl(notify_fd, SECCOMP_IOCTL_NOTIF_RECV, &raw mut *req) };

        if ret < 0 {
            // TODO: is the task terminated or is the syscall interrupted?
            match unsafe { *libc::__errno_location() } {
                libc::EINTR => continue,
                libc::ENOENT => return Ok(()),
                other => return Err(io::Error::from_raw_os_error(other)),
            }
        }

        resp.id = req.id;
        resp.flags = libc::SECCOMP_USER_NOTIF_FLAG_CONTINUE as _;

        let path_remote_ptr = if libc::c_long::from(req.data.nr) == libc::SYS_openat {
            req.data.args[1]
        } else {
            req.data.args[0]
        };

        let local_iov = libc::iovec {
            iov_base: path_buf.as_mut_ptr().cast(),
            iov_len: path_buf.len(),
        };

        let remote_iov = libc::iovec {
            iov_base: path_remote_ptr as _,
            iov_len: path_buf.len(),
        };

        let read_size =
            unsafe { libc::process_vm_readv(req.pid as _, &local_iov, 1, &remote_iov, 1, 0) };
        let Ok(read_size) = usize::try_from(read_size) else {
            let err = io::Error::last_os_error();

            if err.raw_os_error() == Some(libc::ESRCH) {
                // the process is terminated
                continue;
            } else {
                return Err(err);
            };
        };
        let ret = unsafe { libc::ioctl(notify_fd, SECCOMP_IOCTL_NOTIF_SEND, &raw mut *resp) };
        if ret < 0 && unsafe { *libc::__errno_location() } == libc::ENOENT {
            continue;
        }
        check_nonnegative(ret)?;
        let path = CStr::from_bytes_until_nul(&path_buf[..read_size])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        on_path(path.to_bytes());
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let (receiver, sender) = UnixStream::pair()?;
    let sender_fd = sender.as_raw_fd();
    let receiver_fd = receiver.as_raw_fd();
    let mut args = args();
    args.next().unwrap();
    let program = args.next().unwrap();
    let mut command = Command::new(program);
    command.args(args);

    unsafe {
        command.pre_exec(move || {
            let mut sender = unsafe { UnixStream::from_raw_fd(sender_fd) };
            drop(unsafe { UnixStream::from_raw_fd(receiver_fd) });

            check_nonnegative(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0))?;
            let filter = SeccompFilter::new(
                [
                    (libc::SYS_openat, vec![]),
                    #[cfg(target_arch = "x86_64")]
                    (libc::SYS_mkdir, vec![]),
                    #[cfg(target_arch = "x86_64")]
                    (libc::SYS_open, vec![]),
                ]
                .into_iter()
                .collect(),
                SeccompAction::Allow,
                SeccompAction::Raw(linux_raw_sys::ptrace::SECCOMP_RET_USER_NOTIF),
                current_target_arch(),
            )
            .unwrap();

            let mut filter = BpfProgram::try_from(filter).unwrap();

            let mut prog = libc::sock_fprog {
                len: filter.len() as _,
                filter: filter.as_mut_ptr().cast(),
            };

            let notify_fd = seccomp(
                libc::SECCOMP_SET_MODE_FILTER,
                libc::SECCOMP_FILTER_FLAG_NEW_LISTENER as _,
                (&raw mut prog).cast(),
            );
            if notify_fd == -1 {
                return Err(io::Error::last_os_error());
            }
            sender.send_fd(notify_fd)?;
            Ok(())
        })
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<io::Result<Vec<u8>>>();
    spawn(move || {
        let notify_fd = receiver.recv_fd();
        drop(receiver);
        drop(sender);

        if let Err(err) = notify_fd.and_then(|notify_fd| unsafe {
            collect_paths(notify_fd, |path| {
                let _ = tx.send(Ok(path.to_vec()));
            })
        }) {
            let _ = tx.send(Err(err));
        }
    });

    let mut status_fut = command.status();
    let mut paths = HashSet::<Vec<u8>>::new();
    while let Some(item) = rx.recv().await {
        let path = item?;
        paths.insert(path);
    }
    // loop {
    //     let path_fut = pin!(rx.recv());

    //     match select(&mut status_fut, path_fut).await {
    //         Either::Left((status_res, _)) => {
    //             dbg!(status_res);
    //             break;
    //             // handle_notify.await;
    //         }
    //         Either::Right((path_res, status_fut)) => match path_res {
    //             None => {
    //                 dbg!(status_fut.await);
    //                 break;
    //             }
    //             Some(Err(err)) => return Err(err),
    //             Some(Ok(path)) => {
    //                 eprintln!("path: {:?}", from_utf8(&path).unwrap());
    //                 paths.insert(path);
    //             }
    //         },
    //     }
    // }
    for path in &paths {
        if path.first().copied() != Some(b'/') {
            println!("{}", from_utf8(path).unwrap());
        }
    }
    dbg!(paths.len());
    dbg!((status_fut.await));
    Ok(())
}
