// use std::{os::unix::process::CommandExt};

// use tokio::process::Command;

mod consts;

use std::{
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{self, Write},
    mem::ManuallyDrop,
    os::{
        fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
        unix::{ffi::OsStrExt, process::CommandExt},
    },
    path::PathBuf,
    sync::{Arc, LazyLock},
    task::Poll,
};

use crate::FileSystemAccess;
use consts::{ENVNAME_HOST_FD, ENVNAME_PROGRAM, ENVNAME_SOCK_FD, ENVNAME_BOOTSTRAP};

use futures_util::{stream::poll_fn, Stream, TryStream, TryStreamExt};
use nix::{
    fcntl::{fcntl, FcntlArg, FdFlag, OFlag},
    sys::{
        memfd::{memfd_create, MemFdCreateFlag},
        prctl::set_no_new_privs,
        socket::{getsockopt, sockopt::SndBuf},
    },
};
use tokio::process::Command;
use tokio_seqpacket::UnixSeqpacket;

const EXECVE_HOST_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux_execve_host"));

// const EXECVE_HOST_FD: LazyLock<RawFd> = LazyLock::new(|| {
//     let memfd = memfd_create(c"fspy_execve_host", MemFdCreateFlag::empty()).unwrap();
//     OwnedFd::from(memfd).into_raw_fd()
// });

struct Spy {
    execve_host_memfd: Arc<OwnedFd>,
}

fn unset_fd_flag(fd: RawFd, flag_to_remove: FdFlag) -> io::Result<()> {
    fcntl(
        fd,
        FcntlArg::F_SETFD({
            let mut fd_flag = FdFlag::from_bits_retain(fcntl(fd, FcntlArg::F_GETFD)?);
            fd_flag.remove(flag_to_remove);
            fd_flag
        }),
    )?;
    Ok(())
}
fn unset_fl_flag(fd: RawFd, flag_to_remove: OFlag) -> io::Result<()> {
    fcntl(
        fd,
        FcntlArg::F_SETFL({
            let mut fd_flag = OFlag::from_bits_retain(fcntl(fd, FcntlArg::F_GETFL)?);
            fd_flag.remove(flag_to_remove);
            fd_flag
        }),
    )?;
    Ok(())
}

impl Spy {
    fn init() -> io::Result<Self> {
        let execve_host_memfd = memfd_create(c"fspy_execve_host", MemFdCreateFlag::MFD_CLOEXEC)?;
        let mut execve_host_memfile = File::from(execve_host_memfd);
        execve_host_memfile.write_all(EXECVE_HOST_BINARY)?;
        Ok(Self {
            execve_host_memfd: Arc::new(OwnedFd::from(execve_host_memfile)),
        })
    }
    fn new_command<S: AsRef<OsStr>>(
        &self,
        program: S,
        config_fn: impl FnOnce(&mut Command) -> io::Result<()>,
    ) -> io::Result<(
        Command,
        impl TryStream<Item = io::Result<FileSystemAccess>, Ok = FileSystemAccess, Error = io::Error>,
    )> {
        let execve_host_rawfd = self.execve_host_memfd.as_raw_fd();
        let mut command = Command::new(format!("/proc/self/fd/{}", execve_host_rawfd));
        let program = program.as_ref();
        command.arg0(program);

        config_fn(&mut command)?;

        let (receiver, sender) = UnixSeqpacket::pair()?;
        let sender = OwnedFd::from(sender);
        let ipc_buf_size = getsockopt(&sender, SndBuf)?;

        command.env(ENVNAME_PROGRAM, program);
        command.env(ENVNAME_HOST_FD, execve_host_rawfd.to_string());
        command.env(ENVNAME_SOCK_FD, sender.as_raw_fd().to_string());
        command.env(ENVNAME_BOOTSTRAP, "1");

        unsafe {
            command.pre_exec({
                let execve_host_memfd = Arc::clone(&self.execve_host_memfd);
                let sender = sender;

                move || {
                    // unset FD_CLOEXEC
                    unset_fd_flag(sender.as_raw_fd(), FdFlag::FD_CLOEXEC)?;
                    unset_fd_flag(execve_host_memfd.as_raw_fd(), FdFlag::FD_CLOEXEC)?;

                    // unset NONBLOCK
                    unset_fl_flag(sender.as_raw_fd(), OFlag::O_NONBLOCK)?;

                    // setup seccomp
                    set_no_new_privs()?;
                    Ok(())
                }
            });
        }

        let mut buffer = vec![0u8; ipc_buf_size];
        Ok((
            command,
            poll_fn(move |cx| {
                let size = match receiver.poll_recv(cx, &mut buffer) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(err)) => return Poll::Ready(Some(Err(err))),
                    Poll::Ready(Ok(0)) => return Poll::Ready(None),
                    Poll::Ready(Ok(size)) => size,
                };
                let path = PathBuf::from(OsStr::from_bytes(&buffer[..size]));
                Poll::Ready(Some(Ok(FileSystemAccess { path })))
            }),
        ))
    }
    // fn track(&self, command: &mut Command) {
    //     let program = CString::new(command.get_program().as_bytes()).unwrap();

    //     let execve_host_memfd = Arc::clone(&self.execve_host_memfd);
    //     unsafe {
    //         command.pre_exec(move || {
    //             let execve_host_memfd = execve_host_memfd.as_raw_fd();
    //             // fcntl(fd, arg)
    //             // libc::fexecve(fd, argv, envp);
    //             // libc::fexecve(execve_host_memfd.as_raw_fd(), argv, libc::envir)
    //             Err(io::Error::last_os_error())
    //         });
    //     }
    //     command.get_program();
    //     command.status();
    // }
}

#[tokio::test]
async fn hello() -> io::Result<()> {
    let spy = Spy::init()?;
    let (mut cmd, mut stream) = spy.new_command("/usr/local/bin/mise", |_| Ok(()))?;
    dbg!(cmd.status().await?.code());
    drop(cmd);
    while let Some(access) = stream.try_next().await? {
        dbg!(access.path);
    }
    Ok(())
}
