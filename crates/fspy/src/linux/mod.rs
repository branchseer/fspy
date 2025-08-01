// use std::{os::unix::process::CommandExt};

// use tokio::process::Command;

mod syscall_handler;

use fspy_shared_unix::{
    payload::{Payload, encode_payload},
    spawn::handle_spawn,
};
use memmap2::Mmap;
use seccomp_unotify::supervisor::supervise;
use std::{
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{self, Write},
    iter,
    mem::ManuallyDrop,
    ops::{ControlFlow, Deref},
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            process::CommandExt,
        },
    },
    path::PathBuf,
    ptr::null_mut,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU8, AtomicU16, Ordering, fence},
    },
    task::Poll,
};
use syscall_handler::SyscallHandler;

use bincode::{borrow_decode_from_slice, error::DecodeError};
use bumpalo::Bump;
use passfd::{FdPassingExt as _, tokio::FdPassingExt as _};

use tokio::{net::UnixStream, process::Child as TokioChild};

// use crate::FileSystemAccess;

use fspy_shared::ipc::{BINCODE_CONFIG, PathAccess};
use futures_util::{
    FutureExt, Stream, TryStream, TryStreamExt,
    future::{BoxFuture, try_join},
    stream::poll_fn,
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

const EXECVE_HOST_BINARY: &[u8] = include_bytes!(env!("CARGO_CDYLIB_FILE_FSPY_PRELOAD_UNIX"));

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
    shm_mmaps: Vec<Mmap>,
}

impl PathAccessIterable {
    pub fn iter(&self) -> impl Iterator<Item = PathAccess<'_>> {
        let accesses_in_arena = self
            .arenas
            .iter()
            .flat_map(|arena| arena.borrow_accesses().iter())
            .copied();

        let accesses_in_shm = self.shm_mmaps.iter().flat_map(|mmap| {
            let buf = mmap.deref();
            let mut position = 0usize;
            iter::from_fn(move || {
                let (flag_buf, data_buf) = buf[position..].split_first()?;
                let atomic_flag = unsafe { AtomicU8::from_ptr((flag_buf as *const u8).cast_mut()) };
                let flag = atomic_flag.load(Ordering::Acquire);
                if flag == 0 {
                    return None;
                };
                fence(Ordering::Acquire);
                let (path_access, decoded_size) =
                    borrow_decode_from_slice::<PathAccess<'_>, _>(data_buf, BINCODE_CONFIG)
                        .unwrap();

                position += decoded_size + 1;

                Some(path_access)
            })
        });
        accesses_in_shm.chain(accesses_in_arena)
    }
}

pub(crate) async fn spawn_impl(mut command: Command) -> io::Result<TrackedChild> {
    let preload_path = format!(
        "/proc/self/fd/{}",
        command.spy_inner.preload_lib_memfd.as_raw_fd()
    );

    let (shm_fd_sender, shm_fd_receiver) = UnixStream::pair()?;

    let shm_fd_sender = shm_fd_sender.into_std()?;
    shm_fd_sender.set_nonblocking(false)?;
    let shm_fd_sender = OwnedFd::from(shm_fd_sender);

    let supervisor = supervise::<SyscallHandler>()?;

    let payload = Payload {
        ipc_fd: shm_fd_sender.as_raw_fd(),

        #[cfg(target_os = "linux")]
        preload_path: format!(
            "/proc/self/fd/{}",
            command.spy_inner.preload_lib_memfd.as_raw_fd()
        ),

        #[cfg(target_os = "linux")]
        seccomp_payload: supervisor.payload,
    };

    let encoded_payload = encode_payload(payload);

    let preload_lib_memfd = Arc::clone(&command.spy_inner.preload_lib_memfd);
    let mut supervisor_pre_exec = supervisor.pre_exec;

    let mut command_info = command.info();
    let mut pre_spawn = handle_spawn(&mut command_info, true, &encoded_payload)?;
    command.set_info(command_info);

    let mut tokio_command = command.into_tokio_command();

    unsafe {
        tokio_command.pre_exec(move || {
            unset_fd_flag(preload_lib_memfd.as_fd(), FdFlag::FD_CLOEXEC)?;
            unset_fd_flag(shm_fd_sender.as_fd(), FdFlag::FD_CLOEXEC)?;
            supervisor_pre_exec.run()?;
            if let Some(pre_spawn) = &mut pre_spawn {
                pre_spawn.run()?;
            }
            Ok(())
        });
    }
    // let unotify_loop = install_handler::<SyscallHandler>(&mut command)?;

    let child = tokio_command.spawn()?;
    // drop channel_sender in the parent process,
    // so that channel_receiver reaches eof as soon as the last descendant process exits.
    drop(tokio_command);

    let arenas_future = async move {
        let handlers = supervisor.handling_loop.await?;
        let arenas = handlers
            .into_iter()
            .map(|handler| handler.arena)
            .collect::<Vec<_>>();
        io::Result::Ok(arenas)
    };

    let shm_future = async move {
        let mut shm_fds = Vec::<OwnedFd>::new();
        loop {
            let shm_fd = match shm_fd_receiver.recv_fd().await {
                Ok(fd) => unsafe { OwnedFd::from_raw_fd(fd) },
                Err(err) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        break;
                    } else {
                        return Err(err);
                    }
                }
            };
            // let mmap = unsafe { Mmap::map(&fd)? };
            shm_fds.push(shm_fd);
        }
        io::Result::Ok(shm_fds)
    };

    let accesses_future = async move {
        let (arenas, shm_fds) = try_join(arenas_future, shm_future).await?;
        let shm_mmaps = shm_fds
            .into_iter()
            .map(|fd| unsafe { Mmap::map(&fd) })
            .collect::<io::Result<Vec<Mmap>>>()?;
        Ok(PathAccessIterable { arenas, shm_mmaps })
    }
    .boxed();

    Ok(TrackedChild {
        tokio_child: child,
        accesses_future,
    })
}

impl SpyInner {
    pub fn init() -> io::Result<Self> {
        let preload_lib_memfd = memfd_create("fspy_preload", MFdFlags::MFD_CLOEXEC)?;
        let mut execve_host_memfile = File::from(preload_lib_memfd);
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
