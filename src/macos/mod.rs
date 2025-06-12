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
};

use std::process::{Command as StdCommand, Child as StdChild};

use allocator_api2::{
    SliceExt,
    alloc::{Allocator, Global},
    vec::{self, Vec},
};
use bincode::config;
use bumpalo::Bump;

use fspy_shared::{
    ipc::PathAccess,
    macos::{encode_payload, inject::{inject, PayloadWithEncodedString}, Fixtures, Payload},
};
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
    process::{Child as TokioChild, Command as TokioCommand},
};

use crate::Command;

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
pub struct Child {
    pub tokio_child: TokioChild,
    pub path_access_stream: PathAccessStream,
}

pub struct PathAccessStream {
    
}

impl PathAccessStream {
    pub async fn next(&mut self) -> io::Result<Option<PathAccess<'_>>> {
        Ok(todo!())
    }
}


#[derive(Debug, Clone)]
pub(crate) struct SpyInner {
    fixtures: Fixtures,
}

impl SpyInner {
    pub fn init_in_dir(path: &Path) -> io::Result<Self> {
        let coreutils = fixtures::COREUTILS_BINARY.write_to(&path, "")?;
        let _bash_path = fixtures::BRUSH_BINARY.write_to(&path, "")?;
        let interpose_cdylib = fixtures::INTERPOSE_CDYLIB
            .write_to(&path, ".dylib")?;

        let fixtures = Fixtures {
            bash_path: Path::new(
                "/Users/patr0nus/Downloads/oils-for-unix-0.29.0/_bin/cxx-opt-sh/oils-for-unix",
            )
            .into(), //Path::new("/opt/homebrew/bin/bash"),//brush.as_path(),
            coreutils_path: coreutils.as_path().into(),
            interpose_cdylib_path: interpose_cdylib.as_path().into(),
        };
        Ok(Self {
            fixtures
        })
    }
    pub fn spawn_with(self, mut command: Command, with: impl Fn(&mut StdCommand) -> io::Result<StdChild>) -> io::Result<(TokioChild, PathAccessStream)> {
        let ipc_fd = 528491;
        let payload = Payload {
            ipc_fd, fixtures: self.fixtures
        };
        let payload_string = encode_payload(&payload);
        let payload_with_str = PayloadWithEncodedString {
            payload,
            payload_string,
        };
        let bump = Bump::new();
        command.with_info(&bump, |cmd_info| {
            inject(&bump, cmd_info, &payload_with_str);
        });
        todo!()
    }
}

// pub fn spy(
//     program: impl AsRef<OsStr>,
//     cwd: Option<impl AsRef<OsStr>>,
//     arg0: Option<impl AsRef<OsStr>>,
//     args: impl IntoIterator<Item = impl AsRef<OsStr>>,
//     envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
// ) -> io::Result<(
//     impl Future<Output = io::Result<ExitStatus>>,
//     PathAccessStream,
// )> {
//     let tmp_dir = temp_dir().join("fspy");
//     let _ = create_dir(&tmp_dir);

//     let ipc_datagram =
//         tempfile::Builder::new().make_in(&tmp_dir, |path| UnixDatagram::bind(path))?;

//     let ipc_fd_string = ipc_datagram.path().to_path_buf();

//     let acc_buf_size = getsockopt(ipc_datagram.as_file(), SndBuf).unwrap();

//     let coreutils = fixtures::COREUTILS_BINARY.write_to(&tmp_dir, "").unwrap();
//     let brush = fixtures::BRUSH_BINARY.write_to(&tmp_dir, "").unwrap();
//     let interpose_cdylib = fixtures::INTERPOSE_CDYLIB
//         .write_to(&tmp_dir, ".dylib")
//         .unwrap();

//     let program = which::which(program).unwrap();
//     let mut bump = Bump::new();

//     let mut arg_vec = Vec::new_in(&bump);

//     let arg0 = if let Some(arg0) = arg0.as_ref() {
//         Some(arg0.as_ref())
//     } else {
//         None
//     };

//     arg_vec.push(arg0.unwrap_or(program.as_os_str()));
//     arg_vec.extend(
//         args.into_iter()
//             .map(|arg| alloc_os_str(&bump, arg.as_ref())),
//     );

//     let mut env_vec = Vec::new_in(&bump);
//     for (name, value) in envs {
//         let name = alloc_os_str(&bump, name.as_ref());
//         // let name = OsStr::from_bytes(SliceExt::to_vec_in(name, &bump).leak());
//         let value = alloc_os_str(&bump, value.as_ref());
//         env_vec.push((name, value));
//     }
//     let mut cmd = command::Command::<'_, &Bump> {
//         program: program.as_path(),
//         args: arg_vec,
//         envs: env_vec,
//     };

//     let context = Context {
//         ipc_fd: ipc_fd_string.as_os_str(),
//         bash: Path::new(
//             "/Users/patr0nus/Downloads/oils-for-unix-0.29.0/_bin/cxx-opt-sh/oils-for-unix",
//         ), //Path::new("/opt/homebrew/bin/bash"),//brush.as_path(),
//         coreutils: coreutils.as_path(),
//         interpose_cdylib: interpose_cdylib.as_path(),
//     };

//     command::interpose_command(&bump, &mut cmd, context).unwrap();

//     let mut os_cmd = TokioCommand::new(cmd.program);
//     os_cmd
//         .arg0(cmd.args[0])
//         .args(&cmd.args[1..])
//         .env_clear()
//         .envs(cmd.envs.iter().copied());

//     if let Some(cwd) = cwd {
//         os_cmd.current_dir(cwd.as_ref());
//     }

//     let status_fut = os_cmd.status();

//     drop(cmd);
//     drop(os_cmd);

//     bump.reset();

//     Ok((
//         status_fut,
//         todo!(),
//     ))
// }

// pub struct Spy {}
