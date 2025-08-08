// use std::{os::unix::process::CommandExt};

// use tokio::process::Command;

mod syscall_handler;

use fspy_shared_unix::{
    exec::ExecResolveConfig,
    payload::{Payload, encode_payload},
    spawn::handle_exec,
};
use memmap2::Mmap;
use seccomp_unotify::supervisor::supervise;
use std::{
    cell::RefCell,
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{self, Write},
    iter,
    mem::ManuallyDrop,
    ops::{ControlFlow, Deref, DerefMut},
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
use futures_util::{FutureExt, future::try_join};
use nix::{
    fcntl::{FcntlArg, FdFlag, OFlag, fcntl},
    sys::memfd::{MFdFlags, memfd_create},
};

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

// https://github.com/nodejs/node/blob/5794e644b724c6c6cac02d306d87a4d6b78251e5/deps/uv/src/unix/core.c#L803-L808
fn duplicate_until_safe(mut fd: OwnedFd) -> io::Result<OwnedFd> {
    let mut fds: Vec<OwnedFd> = vec![];
    const SAFE_FD_NUM: RawFd = 17;
    while fd.as_raw_fd() < SAFE_FD_NUM {
        let new_fd = fd.try_clone()?;
        fds.push(fd);
        fd = new_fd;
    }
    Ok(fd)
}

pub(crate) async fn spawn_impl(mut command: Command) -> io::Result<TrackedChild> {
    let (shm_fd_sender, shm_fd_receiver) = UnixStream::pair()?;

    let shm_fd_sender = shm_fd_sender.into_std()?;
    shm_fd_sender.set_nonblocking(false)?;
    let shm_fd_sender = duplicate_until_safe(OwnedFd::from(shm_fd_sender))?;

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

    let mut exec = command.get_exec();
    let exec_path_accesses = RefCell::new(PathAccessArena::default());
    let mut pre_exec = handle_exec(
        &mut exec,
        ExecResolveConfig::search_path_enabled(None),
        &encoded_payload,
        |path_access| {
            exec_path_accesses.borrow_mut().add(path_access);
        },
    )?;
    let exec_path_accesses = exec_path_accesses.into_inner();
    command.set_exec(exec);

    let mut tokio_command = command.into_tokio_command();

    unsafe {
        tokio_command.pre_exec(move || {
            unset_fd_flag(preload_lib_memfd.as_fd(), FdFlag::FD_CLOEXEC)?;
            unset_fd_flag(shm_fd_sender.as_fd(), FdFlag::FD_CLOEXEC)?;
            supervisor_pre_exec.run()?;
            if let Some(pre_exec) = &mut pre_exec {
                pre_exec.run()?;
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
        let arenas = std::iter::once(exec_path_accesses)
            .chain(handlers.into_iter().map(|handler| handler.arena))
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

        let preload_lib_memfd = duplicate_until_safe(OwnedFd::from(execve_host_memfile))?;
        Ok(Self {
            preload_lib_memfd: Arc::new(preload_lib_memfd),
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
