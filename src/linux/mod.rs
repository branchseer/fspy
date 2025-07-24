// use std::{os::unix::process::CommandExt};

// use tokio::process::Command;

mod syscall_handler;

use std::{
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{self, Write},
    mem::ManuallyDrop,
    ops::ControlFlow,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            process::CommandExt,
        },
    },
    path::PathBuf,
    ptr::null_mut,
    sync::{Arc, LazyLock},
    task::Poll,
};
use syscall_handler::SyscallHandler;

use allocator_api2::vec::Vec;
use bincode::error::DecodeError;
use bumpalo::Bump;
use passfd::{FdPassingExt as _, tokio::FdPassingExt as _};
use seccomp_unotify::install_handler;
use tokio::{net::UnixStream, process::Child as TokioChild};

// use crate::FileSystemAccess;

use fspy_shared::{
    ipc::{BINCODE_CONFIG, PathAccess},
    linux::{
        EXECVE_HOST_NAME, Payload,
        inject::{PayloadWithEncodedString, inject},
    },
    unix::env::encode_env,
};
use futures_util::{
    FutureExt, Stream, TryStream, TryStreamExt, future::BoxFuture, stream::poll_fn,
};
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

use crate::{Command, TrackedChild, arena::PathAccessArena};

const EXECVE_HOST_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fspy_interpose"));

#[derive(Debug, Clone)]
pub struct SpyInner {
    preload_lib_memfd: Arc<OwnedFd>,
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

pub struct PathAccessIterable {
    arenas: Vec<PathAccessArena>,
}

impl PathAccessIterable {
    pub fn iter(&self) -> impl Iterator<Item = PathAccess<'_>> {
        self.arenas
            .iter()
            .flat_map(|arena| arena.borrow_accesses().iter())
            .copied()
    }
}

pub(crate) async fn spawn_impl(mut command: Command) -> io::Result<TrackedChild> {
    let preload_lib_path = format!(
        "/proc/self/fd/{}",
        command.spy_inner.preload_lib_memfd.as_raw_fd()
    );

    let (sender, receiver) = UnixStream::pair()?;

    let sender = sender.into_std()?;
    sender.set_nonblocking(false)?;
    let sender = OwnedFd::from(sender);

    let payload = Payload {
        preload_lib_path,
        ipc_fd: sender.as_raw_fd(),
    };
    
    let payload_with_str = PayloadWithEncodedString {
        payload_string: encode_env(&payload),
        payload,
    };
    command.resolve_program()?;
    let bump = Bump::new();
    command.with_info(&bump, |cmd_info| {
        inject(&bump, cmd_info, &payload_with_str)?;
        io::Result::Ok(())
    })?;

    let execve_host_memfd = Arc::clone(&command.spy_inner.preload_lib_memfd);
    let mut command = command.into_tokio_command();

    unsafe {
        command.pre_exec(move || {
            // don't close ipc fd on execve
            unset_fd_flag(execve_host_memfd.as_fd(), FdFlag::FD_CLOEXEC)?;
            unset_fd_flag(sender.as_fd(), FdFlag::FD_CLOEXEC)?;
            Ok(())
        });
    }
    let unotify_loop = install_handler::<SyscallHandler>(&mut command)?;

    let child = command.spawn()?;
    // drop channel_sender in the parent process,
    // so that channel_receiver reaches eof as soon as the last descendant process exits.
    drop(command);

    let accesses_future = async move {
        let handlers = unotify_loop.await?;
        let arenas = handlers
            .into_iter()
            .map(|handler| handler.arena)
            .collect::<Vec<_>>();
        io::Result::Ok(PathAccessIterable { arenas })
    }
    .boxed();

    Ok(TrackedChild {
        tokio_child: child,
        accesses_future,
    })
}

impl SpyInner {
    pub fn init() -> io::Result<Self> {
        let execve_host_memfd = memfd_create(EXECVE_HOST_NAME, MFdFlags::MFD_CLOEXEC)?;
        let mut execve_host_memfile = File::from(execve_host_memfd);
        execve_host_memfile.write_all(EXECVE_HOST_BINARY)?;
        Ok(Self {
            preload_lib_memfd: Arc::new(OwnedFd::from(execve_host_memfile)),
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
