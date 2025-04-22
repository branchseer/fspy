use std::{
    ffi::{OsStr, OsString}, fs::File, io::{self, BufRead, Read}, iter::from_fn, os::unix::ffi::{OsStrExt, OsStringExt as _}
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
const PEEK_LIMIT: usize = 128;

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
    mut on_hashbang: impl FnMut(HashBang<'_>)-> io::Result<()>,
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

pub fn parse_hashbang_recursive<'arg, const ARG_CAP: usize, const ARGV_CAP: usize, R: Read, F: FnMut(&OsStr) -> io::Result<R>, O: FnMut(&OsStr) -> io::Result<()>>(
    peek_buf: &mut [u8],
    opts: RecursiveParseOpts,
    reader: R,
    mut get_reader: F,
    arg_buf: &'arg mut ArrayVec<u8, ARG_CAP>,
    argv_out: &mut ArrayVec<&'arg OsStr, ARGV_CAP>,
) -> io::Result<()> {
    let mut recursive_count = 0;
    parse_hashbang_recursive_impl(peek_buf, reader, get_reader, |hashbang| {
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
        recursive_count += 1;
        Ok(())
    })?;
    todo!()
}

// #[cfg(test)]
// mod tests {
//     use std::os::unix::ffi::OsStrExt;

//     use super::*;

//     fn arguments_to_bytes(arguments: Vec<OsString>) -> Vec<Vec<u8>> {
//         arguments.into_iter().map(|arg| arg.into_vec()).collect()
//     }

//     #[test]
//     fn hashbang_basic() {
//         let hashbang = parse_hashbang("#!/bin/sh a\n".as_bytes(), Some(false))
//             .unwrap()
//             .unwrap();
//         assert_eq!(hashbang.interpreter, "/bin/sh");
//         assert_eq!(arguments_to_bytes(hashbang.arguments), &[b"a"]);
//     }

//     #[test]
//     fn hashbang_triming_spaces() {
//         let hashbang = parse_hashbang("#! /bin/sh a \n".as_bytes(), Some(false))
//             .unwrap()
//             .unwrap();
//         assert_eq!(hashbang.interpreter, "/bin/sh");
//         assert_eq!(arguments_to_bytes(hashbang.arguments), &[b"a"]);
//     }

//     #[test]
//     fn hashbang_split_arguments() {
//         let hashbang = parse_hashbang("#! /bin/sh a  b\tc \n".as_bytes(), Some(true))
//             .unwrap()
//             .unwrap();
//         assert_eq!(hashbang.interpreter, "/bin/sh");
//         assert_eq!(arguments_to_bytes(hashbang.arguments), &[b"a", b"b", b"c"]);
//     }
//     #[test]
//     fn hashbang_not_split_arguments() {
//         let hashbang = parse_hashbang("#! /bin/sh  a  b\tc \n".as_bytes(), Some(false))
//             .unwrap()
//             .unwrap();
//         assert_eq!(hashbang.interpreter, "/bin/sh");
//         assert_eq!(arguments_to_bytes(hashbang.arguments), &[b"a  b\tc"]);
//     }

//     #[test]
//     fn hashbang_recursive_basic() {
//         let hashbang = parse_hashbang_recursive(
//             "#!/bin/B".as_bytes(),
//             |path| {
//                 Ok(match path.as_bytes() {
//                     b"/bin/B" => "#! /bin/A param1 param2".as_bytes(),
//                     b"/bin/A" => "not a shebang script".as_bytes(),
//                     _ => unreachable!("Unexpected path: {}", path.display()),
//                 })
//             },
//             Some(false),
//             Some(4),
//         )
//         .unwrap()
//         .unwrap();
//         assert_eq!(hashbang.interpreter, "/bin/A");
//         assert_eq!(hashbang.arguments, &["param1", "param2", "/bin/B"]);
//     }
// }
