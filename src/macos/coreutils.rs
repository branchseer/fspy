use phf::phf_set;

use crate::fixture::{fixture, Fixture};

pub const COREUTILS_BINARY: Fixture = fixture!("coreutils");
pub static COREUTILS_FUNCTIONS: phf::Set<&'static str> = phf_set! {
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

#[cfg(test)]
mod tests {
    use std::{process::Command, str::from_utf8};

    use super::*;

    #[test]
    fn coreutils_functions() {
        let tmpdir = tempfile::tempdir().unwrap();
        let coreutils_path = COREUTILS_BINARY.write_to(&tmpdir).unwrap();
        let output = Command::new(coreutils_path).arg("--list").output().unwrap();
        let mut expected_functions: Vec<&str> = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| {
                let line = line.trim_ascii();
                if line.is_empty() {
                    None
                } else {
                    Some(from_utf8(line).unwrap())
                }
            })
            .collect();
        let mut actual_functions: Vec<&str> = COREUTILS_FUNCTIONS.iter().copied().collect();

        expected_functions.sort_unstable();
        actual_functions.sort_unstable();
        assert_eq!(expected_functions, actual_functions);
    }
}
