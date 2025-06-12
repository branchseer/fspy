use core::slice;
use std::{
    convert::identity,
    env,
    ffi::{CStr, OsStr},
    io::{IoSlice, PipeWriter},
    os::{
        fd::{FromRawFd, OwnedFd, RawFd},
        unix::{ffi::OsStrExt as _, net::UnixDatagram},
    },
    path::Path,
    ptr::null,
    sync::LazyLock,
};

use allocator_api2::vec::Vec;
use bincode::config;
use bstr::BStr;
use bumpalo::Bump;
use smallvec::SmallVec;

use fspy_shared::{
    ipc::{AccessMode, NativeStr, PathAccess},
    macos::{
        PAYLOAD_ENV_NAME, decode_payload,
        inject::{PayloadWithEncodedString, inject},
    },
    unix::cmdinfo::CommandInfo,
};

// use super::command::{CommandInfo, Context, inject};

#[derive(Clone, Copy)]
pub struct RawCommand {
    pub prog: *const libc::c_char,
    pub argv: *const *const libc::c_char,
    pub envp: *const *const libc::c_char,
}

impl RawCommand {
    unsafe fn collect_c_str_array<'a, T>(
        bump: &'a Bump,
        strs: *const *const libc::c_char,
        mut map_fn: impl FnMut(&'a OsStr) -> T,
    ) -> Vec<T, &'a Bump> {
        let mut count = 0usize;
        let mut cur_str = strs;
        while !(unsafe { *cur_str }).is_null() {
            count += 1;
            cur_str = unsafe { cur_str.add(1) };
        }

        let mut str_vec = Vec::<T, &'a Bump>::with_capacity_in(count, bump);
        for i in 0..count {
            let cur_str = unsafe { strs.add(i) };
            str_vec.push(map_fn(OsStr::from_bytes(
                unsafe { CStr::from_ptr(*cur_str) }.to_bytes(),
            )));
        }
        str_vec
    }
    pub fn to_c_str<'a>(bump: &'a Bump, s: &OsStr) -> *const libc::c_char {
        let s = s.as_bytes();
        let mut c_str = Vec::<u8, &'a Bump>::with_capacity_in(s.len() + 1, bump);
        c_str.extend_from_slice(s);
        c_str.push(0);
        c_str.leak().as_ptr().cast()
    }
    fn to_c_str_array<'a>(
        bump: &'a Bump,
        strs: impl ExactSizeIterator<Item = &'a OsStr>,
    ) -> *const *const libc::c_char {
        let mut str_vec =
            Vec::<*const libc::c_char, &'a Bump>::with_capacity_in(strs.len() + 1, bump);
        for s in strs {
            str_vec.push(Self::to_c_str(bump, s));
        }
        str_vec.push(null());
        str_vec.leak().as_ptr().cast()
    }

    pub unsafe fn into_command<'a>(self, bump: &'a Bump) -> CommandInfo<'a, &'a Bump> {
        let program = Path::new(OsStr::from_bytes(
            unsafe { CStr::from_ptr(self.prog) }.to_bytes(),
        ));

        let args = unsafe { Self::collect_c_str_array(bump, self.argv, identity) };

        let envs = unsafe {
            Self::collect_c_str_array(bump, self.envp, |env| {
                let env = env.as_bytes();
                let mut key: &[u8] = env;
                let mut value: &[u8] = b"";
                if let Some(eq_pos) = env.iter().position(|b| *b == b'=') {
                    key = &env[..eq_pos];
                    value = &env[(eq_pos + 1)..];
                }
                (OsStr::from_bytes(key), OsStr::from_bytes(value))
            })
        };

        CommandInfo {
            program,
            args,
            envs,
        }
    }
    fn from_command<'a>(bump: &'a Bump, cmd: &CommandInfo<'a, &'a Bump>) -> Self {
        RawCommand {
            prog: Self::to_c_str(bump, cmd.program.as_os_str()),
            argv: Self::to_c_str_array(bump, cmd.args.iter().copied()),
            envp: Self::to_c_str_array(
                bump,
                cmd.envs.iter().copied().map(|(name, value)| {
                    let name = name.as_bytes();
                    let value = value.as_bytes();
                    let mut env = Vec::<u8, &'a Bump>::with_capacity_in(
                        name.len() + 1 + value.len() + 1,
                        bump,
                    );
                    env.extend_from_slice(name);
                    env.push(b'=');
                    env.extend_from_slice(value);
                    env.push(0);
                    OsStr::from_bytes(env.leak())
                }),
            ),
        }
    }
}

pub struct Client {
    payload_with_str: PayloadWithEncodedString,
}

impl Client {
    fn from_env() -> Self {
        let payload_string = env::var_os(PAYLOAD_ENV_NAME).unwrap();
        let payload = decode_payload(&payload_string);
        Self {
            payload_with_str: PayloadWithEncodedString {
                payload,
                payload_string,
            },
        }
        // let ipc: &'static OsStr = env::var_os("FSPY_IPC_FD").unwrap().leak();
        // let interpose_cdylib = Path::new(env::var_os("DYLD_INSERT_LIBRARIES").unwrap().leak());
        // let bash = Path::new(env::var_os("FSPY_BASH").unwrap().leak());
        // let coreutils = Path::new(env::var_os("FSPY_COREUTILS").unwrap().leak());
        // let command_context = Context {
        //     ipc_fd: ipc,
        //     interpose_cdylib,
        //     bash,
        //     coreutils,
        // };
        // let ipc_fd = Socket::new(Domain::UNIX, Type::DGRAM, None).unwrap();
        // ipc_fd.connect(&SockAddr::unix(ipc).unwrap()).unwrap();

        // Self {
        //     command_context,
        //     ipc_fd,
        // }
    }
    pub unsafe fn interpose_command(
        &self,
        bump: &Bump,
        raw_command: &mut RawCommand,
    ) -> nix::Result<()> {
        let mut cmd = unsafe { raw_command.into_command(bump) };
        inject(bump, &mut cmd, &self.payload_with_str)?;
        *raw_command = RawCommand::from_command(bump, &cmd);
        Ok(())
    }
    pub fn send(&self, mode: AccessMode, path: &BStr) {
        if path.starts_with(b"/dev/") {
            return;
        }
        let mut msg_buf = SmallVec::<u8, 256>::new();

        let msg = PathAccess {
            mode,
            path: NativeStr::from_bytes(&path),
            dir: None,
        };
        // let msg_size =
        //     bincode::encode_into_std_write(msg, &mut msg_buf, config::standard()).unwrap();
        if *IS_NODE {
            eprintln!("{} {:?}", self.payload_with_str.payload.ipc_fd, msg);
        }
        // if let Err(_err) = self.ipc_fd.send_with_flags(&msg_buf[..msg_size], libc::MSG_WAITALL) {
        //     // https://lists.freebsd.org/pipermail/freebsd-net/2006-April/010308.html
        //     // eprintln!("write err: {:?}, data size: {}", err, msg_size);
        // }
    }
}

static IS_NODE: LazyLock<bool> = LazyLock::new(|| std::env::current_exe().unwrap().as_os_str() == "/Users/patr0nus/.local/share/mise/installs/node/24.1.0/bin/node" );

pub static CLIENT: LazyLock<Client> = LazyLock::new(|| Client::from_env());
