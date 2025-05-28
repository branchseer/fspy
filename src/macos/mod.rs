mod fixtures;
mod command;

use std::{env::{self, temp_dir}, ffi::OsStr, fs::create_dir, io, mem::ManuallyDrop, os::{fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd}, unix::ffi::OsStrExt}, path::Path, sync::Arc};

use allocator_api2::{alloc::{Allocator, Global}, vec::{self, Vec}, SliceExt};
use bumpalo::Bump;
use command::Context;
use futures_util::{stream::poll_fn, Stream, TryStream};
use nix::{fcntl::{fcntl, FcntlArg, FdFlag, OFlag}, sys::socket::{getsockopt, sockopt::SndBuf}};
use tokio::{net::UnixDatagram, process::Command};

use crate::FileSystemAccess;

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

pub async fn debug_example() {
    let (receiver, sender) = UnixDatagram::pair().unwrap();
    let sender = sender.into_std().unwrap();
    sender.set_nonblocking(false).unwrap();
    let ipc_buf_size = getsockopt(&sender, SndBuf).unwrap();

    let sender = Arc::new(OwnedFd::from(sender));

    let ipc_fd = sender.as_raw_fd().to_string();

    let fixture_dir = temp_dir().join("fspy");
    let _ = create_dir(&fixture_dir);

    let coreutils = fixtures::COREUTILS_BINARY.write_to(&fixture_dir).unwrap();
    let brush = fixtures::BRUSH_BINARY.write_to(&fixture_dir).unwrap();
    let interpose_cdylib = fixtures::INTERPOSE_CDYLIB.write_to(&fixture_dir).unwrap();
    dbg!(&interpose_cdylib);
  
    let bump = Bump::new();
    let npm = which::which("npm").unwrap();
    let mut args = Vec::new_in(&bump);
    args.push(npm.as_os_str());
    args.push(OsStr::from_bytes(b"start"));
    // args.push(OsStr::from_bytes(b"lint"));


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
        ipc_fd: OsStr::from_bytes(ipc_fd.as_bytes()),
        bash: brush.as_path(),
        coreutils: coreutils.as_path(),
        interpose_cdylib: interpose_cdylib.as_path(),
    };

    command::interpose_command(&bump, &mut cmd, context).unwrap();

    // dbg!(&cmd);

    let mut tokio_command = tokio::process::Command::new(cmd.program);
    tokio_command.arg0(cmd.args[0]).args(cmd.args.iter().skip(1)).env_clear().envs(cmd.envs);

    tokio_command.current_dir("/Users/patr0nus/code/hello_node");

    unsafe { tokio_command.pre_exec(move || {
        unset_fd_flag(sender.as_fd(), FdFlag::FD_CLOEXEC)?;
        Ok(())
    }) };

    let status_fut = tokio_command.status();

    dbg!(status_fut.await);
}


pub struct Spy {

}
