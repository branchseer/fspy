use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, BufRead, Read},
    iter::from_fn,
    os::unix::ffi::{OsStrExt, OsStringExt as _},
};

use arrayvec::ArrayVec;

#[derive(Debug, Clone, Copy)]
pub struct HashBang<'a> {
    pub interpreter: &'a OsStr,
    pub arguments: Arguments<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct Arguments<'a>(&'a [u8]);
impl<'a> Arguments<'a> {
    pub fn as_one(&self) -> &'a OsStr {
        OsStr::from_bytes(self.0)
    }
    pub fn split(&self) -> impl DoubleEndedIterator<Item = &'a OsStr> + use<'a> {
        self.0.split(|c| is_whitespace(*c)).filter_map(|arg| {
            let arg = arg.trim_ascii();
            if arg.is_empty() {
                None
            } else {
                Some(OsStr::from_bytes(arg))
            }
        })
    }
}

fn is_whitespace(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

// https://lwn.net/Articles/779997/
// The array used to hold the shebang line is defined to be 128 bytes in length
pub const DEFAULT_PEEK_SIZE: usize = 128;

pub fn parse_hashbang<'a>(buf: &mut [u8], mut reader: impl Read) -> io::Result<Option<HashBang>> {
    let mut total_read_size = 0;
    loop {
        let read_size = reader.read(&mut buf[total_read_size..])?;
        if read_size == 0 {
            break;
        }
        total_read_size += read_size;
    }
    let Some(buf) = buf[..total_read_size].strip_prefix(b"#!") else {
        return Ok(None);
    };
    // https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/commit/?id=8099b047ecc4
    let Some(buf) = buf.split(|ch| matches!(*ch, b'\n')).next() else {
        return Err(io::Error::from_raw_os_error(libc::ENOEXEC));
    };

    let buf = buf.trim_ascii();
    let Some(interpreter) = buf.split(|ch| is_whitespace(*ch)).next() else {
        return Ok(None);
    };
    let arguments_buf = buf[interpreter.len()..].trim_ascii_start();
    Ok(Some(HashBang {
        interpreter: OsStr::from_bytes(interpreter),
        arguments: Arguments(arguments_buf),
    }))
}

#[derive(Debug)]
pub struct RecursiveParseOpts {
    pub recursion_limit: usize,
    pub split_arguments: bool,
}

impl Default for RecursiveParseOpts {
    fn default() -> Self {
        Self {
            recursion_limit: 4, // BINPRM_MAX_RECURSION
            split_arguments: false,
        }
    }
}

fn parse_hashbang_recursive_impl<R: Read>(
    buf: &mut [u8],
    reader: R,
    mut get_reader: impl FnMut(&OsStr) -> io::Result<R>,
    mut on_hashbang: impl FnMut(HashBang<'_>) -> io::Result<()>,
) -> io::Result<()> {
    let Some(mut hashbang) = parse_hashbang(buf, reader)? else {
        return Ok(());
    };
    on_hashbang(hashbang)?;
    loop {
        let reader = get_reader(&hashbang.interpreter)?;
        let Some(cur_hashbang) = parse_hashbang(buf, reader)? else {
            break Ok(());
        };
        on_hashbang(cur_hashbang)?;
        hashbang = cur_hashbang;
    }
}

pub fn parse_hashbang_recursive<
    const PEEK_CAP: usize,
    R: Read,
    O: FnMut(&OsStr) -> io::Result<R>,
    C: FnMut(&OsStr) -> io::Result<()>,
>(
    opts: RecursiveParseOpts,
    reader: R,
    open: O,
    mut on_arg_reverse: C,
) -> io::Result<()> {
    let mut peek_buf = [0u8; PEEK_CAP];
    let mut recursive_count = 0;
    parse_hashbang_recursive_impl(&mut peek_buf, reader, open, |hashbang| {
        if recursive_count > opts.recursion_limit {
            return Err(io::Error::from_raw_os_error(libc::ELOOP));
        }
        if opts.split_arguments {
            for arg in hashbang.arguments.split().rev() {
                on_arg_reverse(arg)?;
            }
        } else {
            on_arg_reverse(hashbang.arguments.as_one())?;
        }
        on_arg_reverse(hashbang.interpreter)?;
        recursive_count += 1;
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::ffi::OsStrExt;

    use super::*;

    #[test]
    fn hashbang_basic() {
        let mut buf = [0u8; DEFAULT_PEEK_SIZE];
        let hashbang = parse_hashbang(&mut buf, "#!/bin/sh a b\n".as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(hashbang.interpreter.as_bytes(), b"/bin/sh");
        assert_eq!(hashbang.arguments.as_one().as_bytes(), b"a b");
        assert_eq!(
            hashbang
                .arguments
                .split()
                .map(OsStrExt::as_bytes)
                .collect::<Vec<_>>(),
            vec![b"a", b"b"]
        );
    }

    #[test]
    fn hashbang_triming_spaces() {
        let mut buf = [0u8; DEFAULT_PEEK_SIZE];
        let hashbang = parse_hashbang(&mut buf, "#! /bin/sh a \n".as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(hashbang.interpreter, "/bin/sh");
        assert_eq!(hashbang.arguments.as_one().as_bytes(), b"a");
        assert_eq!(
            hashbang
                .arguments
                .split()
                .map(OsStrExt::as_bytes)
                .collect::<Vec<_>>(),
            vec![b"a"]
        );
    }

    #[test]
    fn hashbang_split_arguments() {
        let mut buf = [0u8; DEFAULT_PEEK_SIZE];
        let hashbang = parse_hashbang(&mut buf, "#! /bin/sh a  b\tc \n".as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(hashbang.interpreter, "/bin/sh");
        assert_eq!(
            hashbang
                .arguments
                .split()
                .map(OsStrExt::as_bytes)
                .collect::<Vec<_>>(),
            &[b"a", b"b", b"c"]
        );
    }
    #[test]
    fn hashbang_recursive_basic() {
        let mut args = Vec::<String>::new();
        parse_hashbang_recursive::<DEFAULT_PEEK_SIZE, _, _, _>(
            RecursiveParseOpts {
                split_arguments: true,
                ..RecursiveParseOpts::default()
            },
            "#!/bin/B bparam".as_bytes(),
            |path| {
                Ok(match path.as_bytes() {
                    b"/bin/B" => "#! /bin/A aparam1 aparam2".as_bytes(),
                    b"/bin/A" => "not a shebang script".as_bytes(),
                    _ => unreachable!("Unexpected path: {}", path.display()),
                })
            },
            |arg| {
                args.push(str::from_utf8(arg.as_bytes()).unwrap().to_owned());
                Ok(())
            },
        )
        .unwrap();
        args.reverse();
        assert_eq!(
            args,
            vec!["/bin/A", "aparam1", "aparam2", "/bin/B", "bparam"]
        );
    }
}
