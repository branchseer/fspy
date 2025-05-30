mod command;
mod fixtures;

use std::{
    env::{self, temp_dir},
    ffi::{OsStr, OsString},
    fs::create_dir,
    future::Future,
    io,
    mem::ManuallyDrop,
    net::Shutdown,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd},
        unix::{ffi::OsStrExt, process::CommandExt as _},
    },
    path::{Path, PathBuf},
    pin::pin,
    process::ExitStatus,
    sync::Arc,
    task::Poll,
    thread::spawn,
};

use crate::consts::PathAccess;
use allocator_api2::{
    SliceExt,
    alloc::{Allocator, Global},
    vec::{self, Vec},
};
use bincode::config;
use bumpalo::Bump;
use command::Context;
use futures_util::{
    Stream, TryStream,
    future::{join, select},
    stream::poll_fn,
};
use libc::PIPE_BUF;
use nix::{
    fcntl::{FcntlArg, FdFlag, OFlag, fcntl},
    sys::socket::{getsockopt, sockopt::SndBuf},
};

use tokio::{
    io::{AsyncReadExt, BufReader, ReadBuf},
    net::{UnixDatagram, unix::pipe::pipe},
    process::Command,
};

use crate::FileSystemAccess;

pub fn update_fd_flag(fd: BorrowedFd<'_>, f: impl FnOnce(&mut FdFlag)) -> io::Result<()> {
    fcntl(
        fd,
        FcntlArg::F_SETFD({
            let mut fd_flag = FdFlag::from_bits_retain(fcntl(fd, FcntlArg::F_GETFD)?);
            // dbg!((fd_flag, FdFlag::FD_CLOEXEC));
            f(&mut fd_flag);
            fd_flag
        }),
    )?;
    Ok(())
}

fn alloc_os_str<'a>(bump: &'a Bump, src: &OsStr) -> &'a OsStr {
    OsStr::from_bytes(SliceExt::to_vec_in(src.as_bytes(), bump).leak())
}

pub struct PathAccessStream {
    bump: Bump,
    ipc_datagram: tempfile::NamedTempFile<UnixDatagram>,
    acc_buf_size: usize,
}

impl PathAccessStream {
    pub fn bump_mut(&mut self) -> &mut Bump {
        &mut self.bump
    }
    pub async fn next(&mut self) -> io::Result<Option<PathAccess<'_>>> {
        let mut msg_buf = Vec::<u8, _>::with_capacity_in(self.acc_buf_size, &self.bump);
        let msg_size = self
            .ipc_datagram
            .as_file()
            .recv_buf(&mut msg_buf.spare_capacity_mut())
            .await?;
        unsafe { msg_buf.set_len(msg_size) };
        let msg_buf = msg_buf.leak();

        let (acc, decode_size): (PathAccess, usize) =
            match bincode::borrow_decode_from_slice(msg_buf, config::standard()) {
                Err(decode_error) => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, decode_error));
                }
                Ok(ok) => ok,
            };
        if decode_size != msg_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("decode_size({}) != msg_size({})", decode_size, msg_size),
            ));
        };
        Ok(Some(acc))
    }
}

pub fn spy(
    program: impl AsRef<OsStr>,
    cwd: Option<impl AsRef<OsStr>>,
    arg0: Option<impl AsRef<OsStr>>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> io::Result<(
    impl Future<Output = io::Result<ExitStatus>>,
    PathAccessStream,
)> {
    let tmp_dir = temp_dir().join("fspy");
    let _ = create_dir(&tmp_dir);

    let ipc_datagram =
        tempfile::Builder::new().make_in(&tmp_dir, |path| UnixDatagram::bind(path))?;

    let ipc_fd_string = ipc_datagram.path().to_path_buf();

    let acc_buf_size = getsockopt(ipc_datagram.as_file(), SndBuf).unwrap();

    let coreutils = fixtures::COREUTILS_BINARY.write_to(&tmp_dir).unwrap();
    let brush = fixtures::BRUSH_BINARY.write_to(&tmp_dir).unwrap();
    let interpose_cdylib = fixtures::INTERPOSE_CDYLIB.write_to(&tmp_dir).unwrap();

    let program = which::which(program).unwrap();
    let mut bump = Bump::new();

    let mut arg_vec = Vec::new_in(&bump);

    let arg0 = if let Some(arg0) = arg0.as_ref() {
        Some(arg0.as_ref())
    } else {
        None
    };

    arg_vec.push(arg0.unwrap_or(program.as_os_str()));
    arg_vec.extend(
        args.into_iter()
            .map(|arg| alloc_os_str(&bump, arg.as_ref())),
    );

    let mut env_vec = Vec::new_in(&bump);
    for (name, value) in envs {
        let name = alloc_os_str(&bump, name.as_ref());
        // let name = OsStr::from_bytes(SliceExt::to_vec_in(name, &bump).leak());
        let value = alloc_os_str(&bump, value.as_ref());
        env_vec.push((name, value));
    }
    let mut cmd = command::Command::<'_, &Bump> {
        program: program.as_path(),
        args: arg_vec,
        envs: env_vec,
    };

    let context = Context {
        ipc_fd: ipc_fd_string.as_os_str(),
        bash: brush.as_path(),
        coreutils: coreutils.as_path(),
        interpose_cdylib: interpose_cdylib.as_path(),
    };

    command::interpose_command(&bump, &mut cmd, context).unwrap();

    let mut os_cmd = Command::new(cmd.program);
    os_cmd
        .arg0(cmd.args[0])
        .args(&cmd.args[1..])
        .env_clear()
        .envs(cmd.envs.iter().copied());

    if let Some(cwd) = cwd {
        os_cmd.current_dir(cwd.as_ref());
    }

    let status_fut = os_cmd.status();

    drop(cmd);
    drop(os_cmd);

    bump.reset();

    Ok((
        status_fut,
        PathAccessStream {
            bump,
            ipc_datagram,
            acc_buf_size,
        },
    ))
}

pub struct Spy {}
