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
        unix::{ffi::OsStrExt, net::UnixDatagram, process::CommandExt as _},
    },
    path::Path,
    process::Command,
    sync::Arc,
    thread::spawn,
};

use allocator_api2::{
    alloc::{Allocator, Global},
    vec::{self, Vec},
    SliceExt,
};
use bumpalo::Bump;
use command::Context;
use futures_util::{future::join, stream::poll_fn, Stream, TryStream};
use nix::{
    fcntl::{fcntl, FcntlArg, FdFlag, OFlag},
    sys::socket::{getsockopt, sockopt::SndBuf},
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

pub fn debug_example() {
    let (receiver, sender) = UnixDatagram::pair().unwrap();

    let fixture_dir = temp_dir().join("fspy");
    let _ = create_dir(&fixture_dir);

    let coreutils = fixtures::COREUTILS_BINARY.write_to(&fixture_dir).unwrap();
    let brush = fixtures::BRUSH_BINARY.write_to(&fixture_dir).unwrap();
    let interpose_cdylib = fixtures::INTERPOSE_CDYLIB.write_to(&fixture_dir).unwrap();

    let mut std_cmd = Command::new("echo");

    std_cmd.current_dir("/Users/patr0nus/code/hello_node");

    let recv_loop = spawn(move || {
        let mut recv_buf = vec![0u8; 24];
        loop {
            println!("receving");
            let msg_size = receiver.recv(&mut recv_buf)?;
            dbg!(msg_size);
            if msg_size == 0 {
                break io::Result::Ok(());
            }
            let msg = &recv_buf[..msg_size];
            let access_mode = msg[0];
            let msg = &msg[1..];
            let path_end = msg.iter().position(|c| *c == 0).unwrap() + 1;
            let path = Path::new(OsStr::from_bytes(&msg[..path_end]));
            let caller = Path::new(OsStr::from_bytes(&msg[(path_end + 1)..]));
            println!("path {} caller {}", path.display(), caller.display());
        }
    });

    let status = std_cmd.status();

    // sender.shutdown(Shutdown::Both).unwrap();
    drop(sender);
    dbg!(status);

    drop(std_cmd);
    let recv_result = recv_loop.join().unwrap();
    dbg!(recv_result);
}

pub struct Spy {}
