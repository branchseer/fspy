use std::{
    ffi::{OsStr, OsString},
    iter::once,
    os::unix::ffi::OsStrExt as _,
    path::Path,
};

use allocator_api2::alloc::Allocator;
use phf::{Set, phf_set};

use super::Payload;
use crate::{
    macos::PAYLOAD_ENV_NAME,
    unix::{
        cmdinfo::{CommandInfo, ensure_env},
        shebang::{NixFileSystem, parse_shebang},
    },
};

pub struct PayloadWithEncodedString {
    pub payload: Payload,
    pub payload_string: OsString,
}

pub fn inject<'a, A: Allocator + Copy + 'a>(
    alloc: A,
    command: &mut CommandInfo<'a, A>,
    playload_with_str: &'a PayloadWithEncodedString,
) -> nix::Result<()> {
    command.parse_shebang(alloc)?;

    // TODO: resolve relative paths (e.g. program `sh` with cwd `/bin`)
    let injectable = if let (Some(parent), Some(file_name)) =
        (command.program.parent(), command.program.file_name())
    {
        if matches!(parent.as_os_str().as_bytes(), b"/bin" | b"/usr/bin") {
            let fixtures = &playload_with_str.payload.fixtures;
            if matches!(file_name.as_bytes(), b"sh" | b"bash") {
                command.program = Path::new(fixtures.bash_path.as_os_str());
                true
            } else if COREUTILS_FUNCTIONS.contains(file_name.as_bytes()) {
                command.program = Path::new(fixtures.coreutils_path.as_os_str());
                true
            } else {
                false
            }
        } else {
            true
        }
    } else {
        true
    };

    const DYLD_INSERT_LIBRARIES: &[u8] = b"DYLD_INSERT_LIBRARIES";
    if injectable {
        ensure_env(
            &mut command.envs,
            OsStr::from_bytes(DYLD_INSERT_LIBRARIES),
            &playload_with_str
                .payload
                .fixtures
                .interpose_cdylib_path
                .as_os_str(),
        )?;
        ensure_env(
            &mut command.envs,
            OsStr::from_bytes(PAYLOAD_ENV_NAME.as_bytes()),
            &playload_with_str.payload_string.as_os_str(),
        )?;
    } else {
        command.envs.retain(|(name, _)| {
            let name = name.as_bytes();
            name != DYLD_INSERT_LIBRARIES && name != PAYLOAD_ENV_NAME.as_bytes()
        });
    }

    Ok(())
}

static COREUTILS_FUNCTIONS: Set<&'static [u8]> = phf_set! {
    b"[", b"arch", b"b2sum", b"b3sum", b"base32", b"base64", b"basename", b"basenc",
    b"cat", b"chgrp", b"chmod", b"chown", b"chroot", b"cksum", b"comm", b"cp", b"csplit",
    b"cut", b"date", b"dd", b"df", b"dir", b"dircolors", b"dirname", b"du", b"echo", b"env",
    b"expand", b"expr", b"factor", b"false", b"fmt", b"fold", b"groups", b"hashsum", b"head",
    b"hostid", b"hostname", b"id", b"install", b"join", b"kill", b"link", b"ln", b"logname",
    b"ls", b"md5sum", b"mkdir", b"mkfifo", b"mknod", b"mktemp", b"more", b"mv", b"nice", b"nl",
    b"nohup", b"nproc", b"numfmt", b"od", b"paste", b"pathchk", b"pinky", b"pr", b"printenv",
    b"printf", b"ptx", b"pwd", b"readlink", b"realpath", b"rm", b"rmdir", b"seq", b"sha1sum",
    b"sha224sum", b"sha256sum", b"sha3-224sum", b"sha3-256sum", b"sha3-384sum", b"sha3-512sum",
    b"sha384sum", b"sha3sum", b"sha512sum", b"shake128sum", b"shake256sum", b"shred", b"shuf",
    b"sleep", b"sort", b"split", b"stat", b"stdbuf", b"stty", b"sum", b"sync", b"tac", b"tail",
    b"tee", b"test", b"timeout", b"touch", b"tr", b"true", b"truncate", b"tsort", b"tty", b"uname",
    b"unexpand", b"uniq", b"unlink", b"uptime", b"users", b"vdir", b"wc", b"who", b"whoami", b"yes",
};

#[doc(hidden)]
pub const COREUTILS_FUNCTIONS_FOR_TEST: &Set<&'static [u8]> = &COREUTILS_FUNCTIONS;
