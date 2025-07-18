#![allow(dead_code)]
#![allow(unused)]

mod abort;
mod alloc;
mod bootstrap;
mod client;
mod handler;
mod params;
mod path;

use std::{
    cell::{SyncUnsafeCell, UnsafeCell},
    env::args_os,
    ffi::{CStr, CString, OsStr},
    fs::File,
    io::Write,
    mem::{ManuallyDrop, MaybeUninit},
    os::{
        self,
        fd::{FromRawFd, RawFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use fspy_shared::{
    linux::{
        Payload,
        inject::PayloadWithEncodedString,
        nul_term::{Env, ThinCStr, find_env, iter_environ},
    },
    unix::{
        env::{decode_env, encode_env},
        shebang::parse_shebang,
    },
};
use lexical_core::parse;

use libc_print::libc_eprintln;
use socket2::Socket;

use client::{Client, init_global_client};

pub const SYSCALL_MAGIC: u64 = 0x900d575CA11; // 'good syscall'

pub fn main() -> ! {
    let mut arg_iter = args_os();
    // [program] [encoded_payload] [args...]
    let program: &OsStr = arg_iter.next().unwrap().leak();
    let mut payload_string = arg_iter.next().unwrap();

    let mut payload: Payload = decode_env(&payload_string);
    let bootstrap = payload.bootstrap;

    if bootstrap {
        // re-encode payload for child processes
        payload.bootstrap = false;
        payload_string = encode_env(&payload);
    }

    unsafe {
        init_global_client(Client {
            program,
            payload_with_str: PayloadWithEncodedString {
                payload,
                payload_string,
            },
        })
    };

    if bootstrap {
        bootstrap::bootstrap().unwrap();
    }

    handler::install_signal_handler().unwrap();

    let mut args: Vec<CString> = vec![];
    for arg in arg_iter {
        args.push(CString::new(arg.into_vec()).unwrap());
    }

    let envs: Vec<CString> = std::env::vars_os()
        .map(|(name, value)| {
            let mut env = name.into_vec();
            env.push(b'=');
            env.extend_from_slice(value.as_bytes());
            CString::new(env).unwrap()
        })
        .collect();

    let result = unsafe { execve::execve(Path::new(program), args.iter(), envs.iter()) }.unwrap();
    match result {}
}
