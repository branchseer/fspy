use std::{
    env::current_dir,
    ffi::{CStr, OsStr},
    os::{fd::BorrowedFd, unix::ffi::OsStrExt as _},
    path::PathBuf,
};

#[cfg(target_os = "linux")]
use bstr::BString;
use bstr::{BStr, ByteSlice};
use fspy_shared::ipc::{AccessMode, NativeStr};
use libc::{c_char, c_int};
use nix::fcntl::FcntlArg;
use nix::unistd::getcwd;
use std::{
    borrow::Cow,
    ffi::{CString, OsString},
    os::{fd::RawFd, unix::ffi::OsStringExt as _},
};

#[cfg(target_os = "linux")]
fn get_fd_path(fd: RawFd) -> nix::Result<PathBuf> {
    if fd == libc::AT_FDCWD {
        return getcwd();
    };
    Ok(nix::fcntl::readlink(
        CString::new(format!("/proc/self/fd/{}", fd))
            .unwrap()
            .as_c_str(),
    )?
    .into())
}

#[cfg(target_os = "macos")]
fn get_fd_path(fd: RawFd) -> nix::Result<OsString> {
    if fd == libc::AT_FDCWD {
        return Ok(getcwd()?.into_os_string().into_vec().into());
    };
    let mut path = std::path::PathBuf::new();
    nix::fcntl::fcntl(
        unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) },
        nix::fcntl::FcntlArg::F_GETPATH(&mut path),
    )?;
    Ok(path.into_os_string().into_vec().into())
}

pub trait ToAbsolutePath {
    unsafe fn to_absolute_path<R, F: FnOnce(&BStr) -> nix::Result<R>>(self, f: F)
    -> nix::Result<R>;
}

pub struct Fd(pub c_int);
impl ToAbsolutePath for Fd {
    unsafe fn to_absolute_path<R, F: FnOnce(&BStr) -> nix::Result<R>>(
        self,
        f: F,
    ) -> nix::Result<R> {
        let path = get_fd_path(self.0)?;
        f(path.as_os_str().as_bytes().as_bstr())
    }
}

pub struct MaybeRelative<'a>(pub NativeStr<'a>);
impl ToAbsolutePath for MaybeRelative<'_> {
    unsafe fn to_absolute_path<R, F: FnOnce(&BStr) -> nix::Result<R>>(
        self,
        f: F,
    ) -> nix::Result<R> {
        let pathname = self.0.as_os_str().as_bytes();
        if pathname.first().copied() == Some(b'/') {
            f(pathname.into())
        } else {
            let mut abs_path = get_fd_path(libc::AT_FDCWD)?;
            if !pathname.is_empty() {
                abs_path.push(OsStr::from_bytes(pathname));
            }
            f(abs_path.as_os_str().as_bytes().as_bstr())
        }
    }
}

pub struct PathAt(pub c_int, pub *const c_char);

impl ToAbsolutePath for PathAt {
    unsafe fn to_absolute_path<R, F: FnOnce(&BStr) -> nix::Result<R>>(
        self,
        f: F,
    ) -> nix::Result<R> {
        let pathname = unsafe { CStr::from_ptr(self.1) }.to_bytes().as_bstr();

        if pathname.first().copied() == Some(b'/') {
            f(pathname.into())
        } else {
            let mut abs_path = get_fd_path(self.0)?;
            if !pathname.is_empty() {
                abs_path.push(OsStr::from_bytes(pathname));
            }
            f(abs_path.as_os_str().as_bytes().as_bstr())
        }
    }
}

impl ToAbsolutePath for *const c_char {
    unsafe fn to_absolute_path<R, F: FnOnce(&BStr) -> nix::Result<R>>(
        self,
        f: F,
    ) -> nix::Result<R> {
        unsafe { PathAt(libc::AT_FDCWD, self).to_absolute_path(f) }
    }
}

pub trait ToAccessMode {
    unsafe fn to_access_mode(self) -> AccessMode;
}

impl ToAccessMode for AccessMode {
    unsafe fn to_access_mode(self) -> AccessMode {
        self
    }
}

pub struct OpenFlags(pub c_int);
impl ToAccessMode for OpenFlags {
    unsafe fn to_access_mode(self) -> AccessMode {
        match self.0 & libc::O_ACCMODE {
            libc::O_RDWR => AccessMode::ReadWrite,
            libc::O_WRONLY => AccessMode::Write,
            _ => AccessMode::Read,
        }
    }
}

pub struct ModeStr(pub *const c_char);
impl ToAccessMode for ModeStr {
    unsafe fn to_access_mode(self) -> AccessMode {
        let mode_str = unsafe { CStr::from_ptr(self.0) }.to_bytes().as_bstr();
        let has_read = mode_str.contains(&b'r');
        let has_write = mode_str.contains(&b'w') || mode_str.contains(&b'a');
        match (has_read, has_write) {
            (false, true) => AccessMode::Write,
            (true, true) => AccessMode::ReadWrite,
            _ => AccessMode::Read,
        }
    }
}
