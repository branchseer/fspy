use std::{ffi::OsStr, fs::File, io, iter::once, os::{fd::RawFd, unix::ffi::OsStrExt}, path::Path};

use allocator_api2::{alloc::Allocator, vec::Vec};
// use futures_util::stream::once;
use phf::{Set, phf_set};

use crate::shebang::{parse_shebang, NixFileSystem};

pub static COREUTILS_FUNCTIONS: Set<&'static [u8]> = phf_set! {
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

#[derive(Debug)]
pub struct Command<'a, A: Allocator> {
    pub program: &'a Path,
    pub args: Vec<&'a OsStr, A>,
    pub envs: Vec<(&'a OsStr, &'a OsStr), A>,
}

// pub fn resolve_shebang<'a, A: Allocator + 'a>(alloc: A, command: &mut Command<'a, A>) -> io::Result<()> {
//     if let Some(shebang) = parse_shebang(alloc, &NixFileSystem::default(), command.program)? {
//         // TODO: check exec permission
//         command.program = shebang.interpreter;
//     }
//     Ok(())
// }

#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub ipc_fd: &'a OsStr,
    pub bash: &'a Path,
    pub coreutils: &'a Path,
    pub interpose_cdylib: &'a Path,
}

fn ensure_env<'a, A: Allocator + 'a>(envs: &mut Vec<(&'a OsStr, &'a OsStr), A>, name: &'a OsStr, value: &'a OsStr) -> bool {
    let existing_value = envs.iter().copied().find_map(|(n, v)| if n == name { Some(v) } else { None });
    if let Some(existing_value) = existing_value {
        return existing_value == value;
    };
    envs.push((name, value));
    true
}

pub fn interpose_command<'a, A: Allocator + Clone + 'a>(alloc: A, command: &mut Command<'a, A>, fixtures: Context<'a>) -> nix::Result<()>  {
    if !(
        ensure_env(&mut command.envs, OsStr::from_bytes(b"DYLD_INSERT_LIBRARIES"), fixtures.interpose_cdylib.as_os_str()) &&
        ensure_env(&mut command.envs, OsStr::from_bytes(b"FSPY_BASH"), fixtures.bash.as_os_str()) &&
        ensure_env(&mut command.envs, OsStr::from_bytes(b"FSPY_COREUTILS"), fixtures.coreutils.as_os_str()) &&
        ensure_env(&mut command.envs, OsStr::from_bytes(b"FSPY_IPC_FD"), fixtures.ipc_fd)
    ) {
        return Err(nix::Error::EINVAL);
    }

    if let Some(shebang) = parse_shebang(alloc, &NixFileSystem::default(), command.program)?  {
        command.args[0] = shebang.interpreter.as_os_str();
        command.args.splice(1..1, shebang.arguments.iter().chain(once(command.program.as_os_str())));
        command.program = shebang.interpreter;
    }

    // TODO: resolve relative paths (e.g. program `sh` with cwd `/bin`)
    let (Some(parent), Some(file_name)) = (command.program.parent(), command.program.file_name()) else {
        return Ok(());
    };
    if !matches!(parent.as_os_str().as_bytes(), b"/bin" | b"/usr/bin") {
        return Ok(());
    }
    
    if matches!(file_name.as_bytes(), b"sh" | b"bash") {
        command.program = fixtures.bash;
    } else if COREUTILS_FUNCTIONS.contains(file_name.as_bytes()) {
        command.program = fixtures.coreutils;
    }
    Ok(())
}
