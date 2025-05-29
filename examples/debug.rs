// use std::{os::unix::net::UnixDatagram, process::Command, thread::spawn, io};

use std::{
    future::ready, io::{self, stdout, Write}, ops::Deref as _, os::{
        fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd},
        unix::{net::UnixDatagram as StdUnixDatagram, process::CommandExt as _},
    }, thread::{sleep, spawn}, time::Duration
};

use fspy::update_fd_flag;
use futures_util::future::join;
use libc::c_int;
use nix::fcntl::{FdFlag, FlockArg};
use tokio::{
    io::{AsyncReadExt, Interest},
    net::{
        unix::{self, pipe::pipe},
        UnixDatagram, UnixSocket, UnixStream,
    },
    process::Command,
};

struct D(String);
impl Drop for D {
    fn drop(&mut self) {
        println!("drop");
    }
}

#[tokio::main]
async fn main() {
    // let (y, mut x) = pipe().unwrap();
    // let mut y = y.into_blocking_fd().unwrap();

    // // y.set_nonblocking(false).unwrap();
    // let mut y = Some(y);

    // let mut cmd = Command::new("sleep");

    // cmd.arg("3");

    // let buf = vec![0u8; 8192];
   
    // unsafe {
    //     cmd.pre_exec(move || {
    //         update_fd_flag(y.as_fd(), |flags|flags.remove(FdFlag::FD_CLOEXEC))?;
    //         Ok(())
    //     });
    // }

    // let status_fut = cmd.status();

    // drop(cmd);

    // let recv_loop = async move {
  
    //     let mut recv_buf = vec![0u8; 8192];
    //     loop {
    //         println!("test receving");
    //         let msg_size = x.read(&mut recv_buf).await?;

    //         println!("test recevied");
    //         dbg!(msg_size);
    //         if true {
    //             break io::Result::Ok(());
    //         }
    //     }
    // };

    // // let mut buf = [0u8; 24];
    // dbg!(join(status_fut, recv_loop).await);
    fspy::debug_example().await;
}
