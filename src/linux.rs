// use std::{os::unix::process::CommandExt};

// use tokio::process::Command;

use std::{
    ffi::CString, io, mem::ManuallyDrop, os::{
        fd::{AsRawFd, OwnedFd},
        unix::{ffi::OsStrExt, process::CommandExt},
    }, process::Command, sync::Arc
};

use nix::{fcntl::fcntl, sys::memfd::{memfd_create, MemFdCreateFlag}};

const EXECVE_HOST_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux_execve_host"));

struct Spy {
    execve_host_memfd: Arc<OwnedFd>,
}

impl Spy {
    fn init() -> io::Result<Self> {
        let execve_host_memfd = Arc::new(memfd_create(c"fspy_execve_host", MemFdCreateFlag::empty())?);
        Ok(Self { execve_host_memfd })
    }
    fn track(&self, command: &mut Command) {
        let program =  CString::new(command.get_program().as_bytes()).unwrap();

        let execve_host_memfd = Arc::clone(&self.execve_host_memfd);
        unsafe {
            command.pre_exec(move || {
                let execve_host_memfd = execve_host_memfd.as_raw_fd();
                // fcntl(fd, arg)
                // libc::fexecve(fd, argv, envp);
                Err(io::Error::last_os_error())
            });
        }
        command.get_program();
        // command.status()
    }
}

#[test]
fn az() {
    let mut cmd = Command::new("aa");
    // cmd.env_clear();
    println!("zxz");
    for env in cmd.get_envs() {
        println!("{:?}", env);
    }

    // let mf = memfd::MemfdOptions::new()
    //     .create("fspy_execve_host")
    //     .unwrap();
    // let raw_fd = AsRawFd::as_raw_fd(&mf);
    // let flags = unsafe { libc::fcntl(raw_fd, libc::F_GETFD) };
    // let is_cloexec = (flags & libc::FD_CLOEXEC) != 0;
    // dbg!(is_cloexec);

    // let mut cmd = Command::new("echo");
    // unsafe {
    //     cmd.pre_exec(move || {
    //         unsafe { libc::fcntl(raw_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) };
    //         Ok(())
    //     });
    // }
    // dbg!(cmd.status());

    // let flags = unsafe { libc::fcntl(raw_fd, libc::F_GETFD) };
    // let is_cloexec = (flags & libc::FD_CLOEXEC) != 0;
    // dbg!(is_cloexec);
}
