use std::{
    ffi::{CStr, OsStr},
    os::{fd::{AsRawFd, BorrowedFd, RawFd}, unix::ffi::OsStrExt},
    path::{Component, Path},
};

use allocator_api2::{alloc::Allocator, vec::Vec};
use bstr::BStr;
use std::io::Write as _;

use crate::linux::abort::abort_with;

// fn fill_grow_in<'a, A: Allocator + 'a>(initial_capacity: usize, alloc: A, mut f: impl FnMut(*mut u8, usize) -> nix::Result<usize>) -> nix::Result<Vec<u8, A>> {
//     let mut buf = Vec::<u8, A>::with_capacity_in(initial_capacity, alloc);
//     loop {
//         let len = f(buf.as_mut_ptr(), buf.capacity())?;
//         if len == buf.capacity() {

//         }
//     }
// }

fn readlink_in<'a, A: Allocator + 'a>(path: &CStr, alloc: A) -> nix::Result<Vec<u8, A>> {
    let mut buf = Vec::<u8, A>::with_capacity_in(256, alloc);
    loop {
        let ret = unsafe { libc::readlink(path.as_ptr(), buf.as_mut_ptr(), buf.capacity()) };
        let Ok(len) = usize::try_from(ret) else {
            return Err(nix::Error::last()); // ret == -1
        };
        if len == buf.capacity() {
            // truncation may have occurred. Double the capacity.
            buf.reserve(buf.capacity() * 2);
        } else {
            unsafe { buf.set_len(len) };
            break;
        }
    }
    Ok(buf)
}

fn get_fd_path_in<'a, A: Allocator + Copy + 'a>(fd: RawFd, alloc: A) -> nix::Result<Vec<u8, A>> {
    let mut proc_fd_symlink =
        Vec::<u8, A>::with_capacity_in("/proc/self/fd/2147483647".len(), alloc);
    proc_fd_symlink
        .write_fmt(format_args!("/proc/self/fd/{}\0", fd))
        .unwrap();
    let proc_fd_symlink = unsafe { CStr::from_bytes_with_nul_unchecked(&proc_fd_symlink) };
    readlink_in(proc_fd_symlink, alloc)
}

fn get_current_dir_in<'a, A: Allocator + 'a>(alloc: A) -> nix::Result<Vec<u8, A>> {
    // https://man7.org/linux/man-pages/man7/signal-safety.7.html
    // `getcwd` isn't safe in signal handlers, but `readlink` is.

    // Use `/proc/thread-self` instead of `/proc/self`
    // because cwd may be per-thread. (See `CLONE_FS` in https://man7.org/linux/man-pages/man2/clone.2.html)
    readlink_in(c"/proc/thread-self/cwd", alloc)
}

fn resolve_path_in<'a, A: Allocator + Copy + 'a>(
    dirfd: RawFd,
    c_pathname: &'a CStr,
    alloc: A,
) -> nix::Result<&'a CStr> {
    let pathname = Path::new(OsStr::from_bytes(c_pathname.to_bytes()));
    if pathname.is_absolute() {
        return Ok(c_pathname)
    }
    let mut dir_path = match dirfd {
        libc::AT_FDCWD => get_current_dir_in(alloc)?,
        _ => get_fd_path_in(dirfd, alloc)?,
    };

    // Paths shouldn't be normalized: https://github.com/rust-lang/rust/issues/14028
    dir_path.push(b'/');
    dir_path.extend_from_slice(pathname.as_os_str().as_bytes());
    dir_path.push(0);
    Ok(unsafe { CStr::from_bytes_with_nul_unchecked(dir_path.leak()) })
}

#[cfg(test)]
mod tests {
    use std::{env::current_dir, ffi::OsStr, os::unix::ffi::OsStrExt as _, path::Path};

    use nix::{fcntl::OFlag, sys::stat::Mode};

    use crate::linux::alloc::with_stack_allocator;

    use super::*;

    #[test]
    fn test_get_current_dir_in() {
        with_stack_allocator(|alloc| {
            let cwd = get_current_dir_in(alloc).unwrap();
            let cwd = Path::new(OsStr::from_bytes(&cwd));
            assert_eq!(cwd, std::env::current_dir().unwrap());
        })
    }

    #[test]
    fn test_resolve_path_basic() -> nix::Result<()> {
        with_stack_allocator(|alloc| {
            let dirfd = nix::fcntl::open("/home", OFlag::O_RDONLY, Mode::empty())?;
            let resolved_path = resolve_path_in(dirfd.as_raw_fd(), c"a/b", alloc)?;
            assert_eq!(resolved_path.to_bytes(), b"/home/a/b");
            let resolved_path = resolve_path_in(dirfd.as_raw_fd(), c"/a/b", alloc)?;
            assert_eq!(resolved_path.to_bytes(), b"/a/b");
            nix::Result::Ok(())
        })
    }

    #[test]
    fn test_resolve_path_cwd() -> nix::Result<()> {
        with_stack_allocator(|alloc| {
            let resolved_path = resolve_path_in(libc::AT_FDCWD, c"a/b", alloc)?;
            let expected_path = current_dir().unwrap().join("a/b");
            assert_eq!(OsStr::from_bytes(resolved_path.to_bytes()), expected_path);
            nix::Result::Ok(())
        })
    }
    #[test]
    fn test_resolve_path_preserve_dots() -> nix::Result<()> {
        with_stack_allocator(|alloc| {
            let dirfd = nix::fcntl::open("/home", OFlag::O_RDONLY, Mode::empty())?;
            let resolved_path = resolve_path_in(dirfd.as_raw_fd(), c"a/./b", alloc)?;
            assert_eq!(resolved_path.to_bytes(), b"/home/a/./b");
            let resolved_path = resolve_path_in(dirfd.as_raw_fd(), c"a/../b", alloc)?;
            assert_eq!(resolved_path.to_bytes(), b"/home/a/../b");
            nix::Result::Ok(())
        })
    }
}
