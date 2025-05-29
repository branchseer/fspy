mod command;
mod fixtures;

use std::{
    env::{self, temp_dir},
    ffi::OsStr,
    fs::create_dir,
    io,
    mem::ManuallyDrop,
    net::Shutdown,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd},
        unix::{ffi::OsStrExt, process::CommandExt as _},
    },
    path::Path,
    pin::pin,
    sync::Arc,
    thread::spawn,
};

use crate::consts::IpcMessage;
use allocator_api2::{
    alloc::{Allocator, Global},
    vec::{self, Vec},
    SliceExt,
};
use bincode::config;
use bumpalo::Bump;
use command::Context;
use futures_util::{
    future::{join, select},
    stream::poll_fn,
    Stream, TryStream,
};
use libc::PIPE_BUF;
use nix::{
    fcntl::{fcntl, FcntlArg, FdFlag, OFlag},
    sys::socket::{getsockopt, sockopt::SndBuf},
};
use tokio::{
    io::{AsyncReadExt, BufReader},
    net::{unix::pipe::pipe, UnixDatagram},
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

pub async fn debug_example() {
    // let (pipe_sender, pipe_receiver) = pipe().unwrap();
    // let pipe_sender = pipe_sender.into_blocking_fd().unwrap();
    let tmp_dir = temp_dir().join("fspy");
    let _ = create_dir(&tmp_dir);

    let temp_unix_datagram = tempfile::Builder::new()
        .make_in(&tmp_dir, |path| {
            dbg!(path);
            UnixDatagram::bind(path)
        })
        .unwrap();

    let send_buf_size = getsockopt(&temp_unix_datagram, SndBuf).unwrap();

    let ipc_fd_string = temp_unix_datagram.path().to_path_buf();

    let coreutils = fixtures::COREUTILS_BINARY.write_to(&tmp_dir).unwrap();
    let brush = fixtures::BRUSH_BINARY.write_to(&tmp_dir).unwrap();
    let interpose_cdylib = fixtures::INTERPOSE_CDYLIB.write_to(&tmp_dir).unwrap();

    let bump = Bump::new();
    let npm = which::which("npm").unwrap();
    let mut args = Vec::new_in(&bump);
    args.push(npm.as_os_str());
    args.push(OsStr::from_bytes(b"run"));
    args.push(OsStr::from_bytes(b"start"));

    let mut envs = Vec::new_in(&bump);
    for (name, value) in env::vars_os() {
        let name = name.as_os_str().as_bytes();
        let name = OsStr::from_bytes(SliceExt::to_vec_in(name, &bump).leak());
        let value = value.as_os_str().as_bytes();
        let value = OsStr::from_bytes(SliceExt::to_vec_in(value, &bump).leak());

        envs.push((name, value));
    }
    let mut cmd = command::Command::<'_, &Bump> {
        program: npm.as_path(),
        args,
        envs,
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
        .envs(cmd.envs);

    os_cmd.current_dir("/Users/patr0nus/code/hello_node");

    let status = os_cmd.status();

    drop(os_cmd);

    let recv_loop = async move {
        let mut recv_buf = vec![0u8; send_buf_size];
        loop {
            let msg_size = temp_unix_datagram.as_file().recv(&mut recv_buf).await?;
            if msg_size == 0 {
                return io::Result::Ok(());
            }
            let msg_buf = &mut recv_buf[..msg_size];

            let (msg, decode_size): (IpcMessage, usize) =
                bincode::borrow_decode_from_slice(msg_buf, config::standard()).unwrap();
            assert_eq!(decode_size, msg_size);

            println!("{:?}", msg);
        }
    };

    let recv_loop = pin!(recv_loop);
    let status = pin!(status);

    let res = select(recv_loop, status).await;
    if let futures_util::future::Either::Right((status, _)) = res {
        dbg!(status);
    } else {
        unreachable!()
    };
}

pub struct Spy {}
