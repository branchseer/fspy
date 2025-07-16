// use std::{os::unix::process::CommandExt};

// use tokio::process::Command;

use std::{
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{self, Write},
    mem::ManuallyDrop,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            process::CommandExt,
        },
    },
    path::PathBuf,
    sync::{Arc, LazyLock},
    task::Poll,
};

use bumpalo::Bump;
use tokio::process::Child as TokioChild;

// use crate::FileSystemAccess;

use fspy_shared::{
    ipc::PathAccess,
    linux::{
        Payload,
        inject::{PayloadWithEncodedString, inject},
    },
    unix::env::encode_env,
};
use futures_util::{Stream, TryStream, TryStreamExt, stream::poll_fn};
use nix::{
    fcntl::{FcntlArg, FdFlag, OFlag, fcntl},
    sys::{
        memfd::{MFdFlags, memfd_create},
        prctl::set_no_new_privs,
        socket::{getsockopt, sockopt::SndBuf},
    },
};

use tokio_seqpacket::UnixSeqpacket;
use which::which;

use crate::Command;

const EXECVE_HOST_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fspy_interpose"));

// const EXECVE_HOST_FD: LazyLock<RawFd> = LazyLock::new(|| {
//     let memfd = memfd_create(c"fspy_execve_host", MemFdCreateFlag::empty()).unwrap();
//     OwnedFd::from(memfd).into_raw_fd()
// });

#[derive(Debug, Clone)]
pub struct SpyInner {
    execve_host_memfd: Arc<OwnedFd>,
}

fn unset_fd_flag(fd: BorrowedFd<'_>, flag_to_remove: FdFlag) -> io::Result<()> {
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
fn unset_fl_flag(fd: BorrowedFd<'_>, flag_to_remove: OFlag) -> io::Result<()> {
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

pub struct PathAccessIter {}

impl PathAccessIter {
    pub async fn next<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<Option<PathAccess<'a>>> {
        Ok(None)
    }
}

pub(crate) async fn spawn_impl(mut command: Command) -> io::Result<(TokioChild, PathAccessIter)> {
    command.resolve_program()?;
    let bump = Bump::new();

    let execve_host_path = format!(
        "/proc/self/fd/{}",
        command.spy_inner.execve_host_memfd.as_raw_fd()
    );
    let payload = Payload {
        execve_host_path: OsStr::from_bytes(execve_host_path.as_bytes()).into(),
        ipc_fd: 0,
        bootstrap: true,
    };
    let payload_with_str = PayloadWithEncodedString {
        payload_string: encode_env(&payload),
        payload,
    };
    command.with_info(&bump, |cmd_info| {
        inject(&bump, cmd_info, &payload_with_str)?;
        io::Result::Ok(())
    })?;

    let execve_host_memfd = Arc::clone(&command.spy_inner.execve_host_memfd);
    let mut command = command.into_tokio_command();

    unsafe {
        command.pre_exec(move || {
            // unset FD_CLOEXEC
            unset_fd_flag(execve_host_memfd.as_fd(), FdFlag::FD_CLOEXEC)?;
            set_no_new_privs()?;
            Ok(())
        });
    }
    let child = command.spawn()?;
    // drop channel_sender in the parent process,
    // so that channel_receiver reaches eof as soon as the last descendant process exits.
    drop(command);

    Ok((child, PathAccessIter {}))
}

impl SpyInner {
    pub fn init() -> io::Result<Self> {
        let execve_host_memfd = memfd_create(c"fspy_execve_host", MFdFlags::MFD_CLOEXEC)?;
        let mut execve_host_memfile = File::from(execve_host_memfd);
        execve_host_memfile.write_all(EXECVE_HOST_BINARY)?;
        Ok(Self {
            execve_host_memfd: Arc::new(OwnedFd::from(execve_host_memfile)),
        })
    }
}

// #[tokio::test]
// async fn hello() -> io::Result<()> {
//     let spy = Spy::init()?;
//     let (mut cmd, mut stream) = spy.new_command("/bin/bash", |cmd| {
//         cmd.args(["-c", "ls / && mise"]);

//         Ok(())
//     })?;
//     dbg!(cmd.status().await?.code());
//     drop(cmd);
//     while let Some(access) = stream.try_next().await? {
//         dbg!(access.path);
//     }
//     Ok(())
// }
