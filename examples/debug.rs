// use std::{os::unix::net::UnixDatagram, process::Command, thread::spawn, io};

use std::{future::ready, io::{self, stdout}, os::{fd::{AsFd, AsRawFd, BorrowedFd}, unix::process::CommandExt as _}, thread::spawn};

use fspy::update_fd_flag;
use futures_util::future::join;
use nix::fcntl::FdFlag;
use tokio::{io::{AsyncReadExt, Interest}, net::{UnixDatagram, UnixStream}, process::Command};

struct D(String);
impl Drop for D {
    fn drop(&mut self) {
        println!("drop");
    }
}

#[tokio::main]
async fn main() {

    let (x, y) = std::os::unix::net::UnixDatagram::pair().unwrap();

    let mut cmd = Command::new("sleep");

    cmd.arg("3");
    let d = D(String::new());
    unsafe {
        cmd.pre_exec(move || {
            let _ = &d;
            let y = &y;
            // update_fd_flag(y.as_fd(), |flag| flag.remove(FdFlag::FD_CLOEXEC))?;
            Ok(())
        });
    }


    let status_fut = cmd.status();

    drop(cmd);


    let recv_loop = spawn(move || {
        let mut recv_buf = vec![0u8; 24];
        loop {
            println!("test receving");
            let msg_size = x.recv(&mut recv_buf)?;

            println!("test recevied");
            dbg!(msg_size);
            if true {
                break io::Result::Ok(());
            }
        }
    });

    // let mut buf = [0u8; 24];
    dbg!(status_fut.await);
    dbg!(recv_loop.join().unwrap());
    // fspy::debug_example();
}
