use assertables::assert_contains;
use nix::fcntl::{AT_FDCWD, OFlag, openat};
use nix::sys::stat::Mode;
use seccomp_unotify::install_handler;
use std::env::set_current_dir;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, Mutex};

use seccomp_unotify::{
    handler::arg::{CStrPtr, Fd},
    impl_handler,
};
use tokio::{process::Command, task::spawn_blocking};

#[derive(Debug, PartialEq, Eq)]
enum Syscall {
    Openat { at_dir: OsString, path: OsString },
}

#[derive(Default)]
struct SyscallRecorder(Arc<Mutex<Vec<Syscall>>>);
impl SyscallRecorder {
    fn openat(&self, (fd, path): (Fd, CStrPtr)) -> io::Result<()> {
        let at_dir = fd.get_path()?;
        let mut path_buf = [0; 1024];
        let path_len = path.read(&mut path_buf)?;
        let path = OsStr::from_bytes(&path_buf[..path_len]).to_os_string();
        self.0
            .lock()
            .unwrap()
            .push(Syscall::Openat { at_dir, path });
        Ok(())
    }
}

impl_handler!(SyscallRecorder, openat);

async fn run_in_pre_exec(
    mut f: impl FnMut() -> io::Result<()> + Send + Sync + 'static,
) -> io::Result<Vec<Syscall>> {
    let syscalls: Arc<Mutex<Vec<Syscall>>> = Default::default();
    let recorder = SyscallRecorder(syscalls.clone());
    let mut cmd = Command::new("/bin/echo");
    let handle_loop = install_handler(&mut cmd, recorder)?;
    unsafe {
        cmd.pre_exec(move || {
            f()?;
            libc::exit(0)
        });
    }
    let child_fut = spawn_blocking(move || cmd.spawn());
    handle_loop.await?;
    child_fut.await.unwrap()?.wait().await?; // lol
    let syscalls = Arc::into_inner(syscalls).expect("handler should have been dropped");
    Ok(syscalls.into_inner().unwrap())
}

#[tokio::test]
async fn fd_and_path() -> io::Result<()> {
    let syscalls = run_in_pre_exec(|| {
        set_current_dir("/")?;
        let home_fd = nix::fcntl::open(c"/home", OFlag::O_PATH, Mode::empty()).unwrap();
        let _ = openat(home_fd, c"open_at_home", OFlag::O_RDONLY, Mode::empty());
        let _ = openat(AT_FDCWD, c"openat_cwd", OFlag::O_RDONLY, Mode::empty());
        Ok(())
    })
    .await?;

    assert_contains!(
        syscalls,
        &Syscall::Openat {
            at_dir: "/".into(),
            path: "/home".into(),
        }
    );
    assert_contains!(
        syscalls,
        &Syscall::Openat {
            at_dir: "/home".into(),
            path: "open_at_home".into(),
        }
    );
    assert_contains!(
        syscalls,
        &Syscall::Openat {
            at_dir: "/".into(),
            path: "openat_cwd".into(),
        }
    );
    Ok(())
}
