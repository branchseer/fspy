use std::{ffi::OsStr, fs::File, io, path::Path};

use allocator_api2::{alloc::Allocator, vec::Vec};
use phf::{Set, phf_set};

use crate::shebang::{parse_shebang};

pub static COREUTILS_FUNCTIONS: Set<&'static str> = phf_set! {
    "[", "arch", "b2sum", "b3sum", "base32", "base64", "basename", "basenc",
    "cat", "chgrp", "chmod", "chown", "chroot", "cksum", "comm", "cp", "csplit",
    "cut", "date", "dd", "df", "dir", "dircolors", "dirname", "du", "echo", "env",
    "expand", "expr", "factor", "false", "fmt", "fold", "groups", "hashsum", "head",
    "hostid", "hostname", "id", "install", "join", "kill", "link", "ln", "logname",
    "ls", "md5sum", "mkdir", "mkfifo", "mknod", "mktemp", "more", "mv", "nice", "nl",
    "nohup", "nproc", "numfmt", "od", "paste", "pathchk", "pinky", "pr", "printenv",
    "printf", "ptx", "pwd", "readlink", "realpath", "rm", "rmdir", "seq", "sha1sum",
    "sha224sum", "sha256sum", "sha3-224sum", "sha3-256sum", "sha3-384sum", "sha3-512sum",
    "sha384sum", "sha3sum", "sha512sum", "shake128sum", "shake256sum", "shred", "shuf",
    "sleep", "sort", "split", "stat", "stdbuf", "stty", "sum", "sync", "tac", "tail",
    "tee", "test", "timeout", "touch", "tr", "true", "truncate", "tsort", "tty", "uname",
    "unexpand", "uniq", "unlink", "uptime", "users", "vdir", "wc", "who", "whoami", "yes",
};

pub struct Command<'a, A> where &'a A: Allocator {
    program: &'a Path,
    args: Vec<&'a OsStr, &'a A>,
    envs: Vec<(&'a OsStr, &'a OsStr), &'a A>,
}

pub fn interpose_command<'a, A: Allocator>(mut command: Command<'a, A>) -> io::Result<()>  {
    let reader = File::open(command.program)?;
    if let Some(shebang) = parse_shebang(*command.args.allocator(), reader)? {
        // TODO: check exec permission
        command.program = Path::new(shebang.interpreter);
    }
    // TODO: resolve relative paths (e.g. program `sh` with cwd `/bin`)
    // let shebang
    Ok(())
}
