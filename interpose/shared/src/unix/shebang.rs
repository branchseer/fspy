use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt as _, path::Path,
};

use allocator_api2::{alloc::Allocator, vec};


#[derive(Debug, Clone, Copy)]
pub struct Shebang<'a> {
    pub interpreter: &'a Path,
    pub arguments: Arguments<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct Arguments<'a> {
    arguments_buf: &'a [u8],
    should_split: bool,
}

impl<'a> Arguments<'a> {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &'a OsStr> + use<'a> {
        let should_split = self.should_split;
        self.arguments_buf.split(move |c| should_split && is_whitespace(*c)).filter_map(|arg| {
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

pub trait FileSystem {
    type Error;
    fn peek_executable(&self, path: &Path, buf: &mut [u8]) -> Result<usize, Self::Error>;
    fn format_error(&self) -> Self::Error;
}

#[derive(Default, Debug)]
pub struct NixFileSystem(());

impl FileSystem for NixFileSystem {
    type Error = nix::Error;

    fn peek_executable(&self, path: &Path, buf: &mut [u8]) -> Result<usize, Self::Error> {
        use nix::{fcntl::{open, OFlag}, sys::stat::Mode};

        eprintln!("opening");
        let fd = dbg!(open(path,  OFlag::O_RDONLY | OFlag::O_CLOEXEC, Mode::empty()))?;
        // TODO: check exec permission
        let mut total_read_size = 0;
        loop {
            let read_size =  nix::unistd::read(&fd, &mut buf[total_read_size..])?;
            if read_size == 0 {
                break;
            }
            total_read_size += read_size;
        }
        Ok(total_read_size)
    }
    
    fn format_error(&self) -> Self::Error {
        // https://github.com/torvalds/linux/blob/5723cc3450bccf7f98f227b9723b5c9f6b3af1c5/fs/binfmt_script.c#L59-L80
        nix::Error::ENOEXEC
    }
}

pub fn parse_shebang<'a, A: Allocator + 'a, FS: FileSystem>(alloc: A, fs: &FS, path: &Path) -> Result<Option<Shebang<'a>>, FS::Error> {
    // https://lwn.net/Articles/779997/
    // > The array used to hold the shebang line is defined to be 128 bytes in length
    // TODO: check linux/macOS' kernel source
    const PEEK_SIZE: usize = 128;

    let buf = vec![in alloc; 0u8; PEEK_SIZE].leak::<'a>();

    let total_read_size = fs.peek_executable(path, buf)?;

    let Some(buf) = buf[..total_read_size].strip_prefix(b"#!") else {
        return Ok(None);
    };
    
    let Some(buf) = buf.split(|ch| matches!(*ch, b'\n')).next() else {
        return Err(fs.format_error());
    };
    let buf = buf.trim_ascii();
    let Some(interpreter) = buf.split(|ch| is_whitespace(*ch)).next() else {
        return Ok(None);
    };
    let arguments_buf = buf[interpreter.len()..].trim_ascii_start();
    Ok(Some(Shebang {
        interpreter: Path::new(OsStr::from_bytes(interpreter)),
        arguments: Arguments {
            arguments_buf,
            // TODO: linux doesn't split arguments in shebang
            should_split: true,
        },
    }))
}

// #[derive(Debug)]
// pub struct RecursiveParseOpts {
//     pub recursion_limit: usize,
//     pub split_arguments: bool,
// }

// impl Default for RecursiveParseOpts {
//     fn default() -> Self {
//         Self {
//             recursion_limit: 4, // BINPRM_MAX_RECURSION
//             split_arguments: false,
//         }
//     }
// }

// fn parse_shebang_recursive_impl<R: Read>(
//     buf: &mut [u8],
//     reader: R,
//     mut get_reader: impl FnMut(&OsStr) -> io::Result<R>,
//     mut on_shebang: impl FnMut(shebang<'_>) -> io::Result<()>,
// ) -> io::Result<()> {
//     let Some(mut shebang) = parse_shebang(buf, reader)? else {
//         return Ok(());
//     };
//     on_shebang(shebang)?;
//     loop {
//         let reader = get_reader(&shebang.interpreter)?;
//         let Some(cur_shebang) = parse_shebang(buf, reader)? else {
//             break Ok(());
//         };
//         on_shebang(cur_shebang)?;
//         shebang = cur_shebang;
//     }
// }

// pub fn parse_shebang_recursive<
//     const PEEK_CAP: usize,
//     R: Read,
//     O: FnMut(&OsStr) -> io::Result<R>,
//     C: FnMut(&OsStr) -> io::Result<()>,
// >(
//     opts: RecursiveParseOpts,
//     reader: R,
//     open: O,
//     mut on_arg_reverse: C,
// ) -> io::Result<()> {
//     let mut peek_buf = [0u8; PEEK_CAP];
//     let mut recursive_count = 0;
//     parse_shebang_recursive_impl(&mut peek_buf, reader, open, |shebang| {
//         if recursive_count > opts.recursion_limit {
//             return Err(io::Error::from_raw_os_error(libc::ELOOP));
//         }
//         if opts.split_arguments {
//             for arg in shebang.arguments.split().rev() {
//                 on_arg_reverse(arg)?;
//             }
//         } else {
//             on_arg_reverse(shebang.arguments.as_one())?;
//         }
//         on_arg_reverse(shebang.interpreter)?;
//         recursive_count += 1;
//         Ok(())
//     })?;
//     Ok(())
// }

// #[cfg(test)]
// mod tests {
//     use std::os::unix::ffi::OsStrExt;

//     use super::*;

//     #[test]
//     fn shebang_basic() {
//         let mut buf = [0u8; PEEK_SIZE];
//         let shebang = parse_shebang(&mut buf, "#!/bin/sh a b\n".as_bytes())
//             .unwrap()
//             .unwrap();
//         assert_eq!(shebang.interpreter.as_bytes(), b"/bin/sh");
//         assert_eq!(shebang.arguments.as_one().as_bytes(), b"a b");
//         assert_eq!(
//             shebang
//                 .arguments
//                 .split()
//                 .map(OsStrExt::as_bytes)
//                 .collect::<Vec<_>>(),
//             vec![b"a", b"b"]
//         );
//     }

//     #[test]
//     fn shebang_triming_spaces() {
//         let mut buf = [0u8; PEEK_SIZE];
//         let shebang = parse_shebang(&mut buf, "#! /bin/sh a \n".as_bytes())
//             .unwrap()
//             .unwrap();
//         assert_eq!(shebang.interpreter, "/bin/sh");
//         assert_eq!(shebang.arguments.as_one().as_bytes(), b"a");
//         assert_eq!(
//             shebang
//                 .arguments
//                 .split()
//                 .map(OsStrExt::as_bytes)
//                 .collect::<Vec<_>>(),
//             vec![b"a"]
//         );
//     }

//     #[test]
//     fn shebang_split_arguments() {
//         let mut buf = [0u8; PEEK_SIZE];
//         let shebang = parse_shebang(&mut buf, "#! /bin/sh a  b\tc \n".as_bytes())
//             .unwrap()
//             .unwrap();
//         assert_eq!(shebang.interpreter, "/bin/sh");
//         assert_eq!(
//             shebang
//                 .arguments
//                 .split()
//                 .map(OsStrExt::as_bytes)
//                 .collect::<Vec<_>>(),
//             &[b"a", b"b", b"c"]
//         );
//     }
//     #[test]
//     fn shebang_recursive_basic() {
//         let mut args = Vec::<String>::new();
//         parse_shebang_recursive::<PEEK_SIZE, _, _, _>(
//             RecursiveParseOpts {
//                 split_arguments: true,
//                 ..RecursiveParseOpts::default()
//             },
//             "#!/bin/B bparam".as_bytes(),
//             |path| {
//                 Ok(match path.as_bytes() {
//                     b"/bin/B" => "#! /bin/A aparam1 aparam2".as_bytes(),
//                     b"/bin/A" => "not a shebang script".as_bytes(),
//                     _ => unreachable!("Unexpected path: {}", path.display()),
//                 })
//             },
//             |arg| {
//                 args.push(str::from_utf8(arg.as_bytes()).unwrap().to_owned());
//                 Ok(())
//             },
//         )
//         .unwrap();
//         args.reverse();
//         assert_eq!(
//             args,
//             vec!["/bin/A", "aparam1", "aparam2", "/bin/B", "bparam"]
//         );
//     }
// }
