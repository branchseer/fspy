use std::{borrow::Cow, os::unix::ffi::OsStrExt, path::absolute};

use fspy_shared::ipc::{AccessMode, PathAccess};
use which::sys::{RealSys, Sys};

use crate::shebang::{NixFileSystem, ShebangParseFileSystem};

struct SysWithCallback<WhichSys, ShebangFS, F> {
    which_sys: WhichSys,
    shebang_fs: ShebangFS,
    callback: F,
}

impl<WhichSys, ShebangFS, F: Fn(PathAccess<'_>)> SysWithCallback<WhichSys, ShebangFS, F> {
    pub fn invoke_callback(&self, path: &std::path::Path, mode: AccessMode) {
        let abs_path = if path.is_absolute() {
            Cow::Borrowed(path)
        } else {
            Cow::Owned(absolute(path).expect("Failed to get current directory"))
        };
        let path_access = PathAccess {
            path: abs_path.as_ref().into(),
            mode,
        };
        (self.callback)(path_access);
    }
}

impl<WhichSys: Sys, ShebangFS, F: Fn(PathAccess<'_>)> Sys
    for SysWithCallback<WhichSys, ShebangFS, F>
{
    type ReadDirEntry = WhichSys::ReadDirEntry;
    type Metadata = WhichSys::Metadata;

    fn is_windows(&self) -> bool {
        self.which_sys.is_windows()
    }

    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        self.which_sys.current_dir()
    }

    fn home_dir(&self) -> Option<std::path::PathBuf> {
        self.which_sys.home_dir()
    }

    fn env_split_paths(&self, paths: &std::ffi::OsStr) -> Vec<std::path::PathBuf> {
        self.which_sys.env_split_paths(paths)
    }

    fn env_path(&self) -> Option<std::ffi::OsString> {
        self.which_sys.env_path()
    }

    fn env_path_ext(&self) -> Option<std::ffi::OsString> {
        self.which_sys.env_path_ext()
    }

    fn metadata(&self, path: &std::path::Path) -> std::io::Result<Self::Metadata> {
        self.invoke_callback(path, AccessMode::Read);
        self.which_sys.metadata(path)
    }

    fn symlink_metadata(&self, path: &std::path::Path) -> std::io::Result<Self::Metadata> {
        self.invoke_callback(path, AccessMode::Read);
        self.which_sys.symlink_metadata(path)
    }

    fn read_dir(
        &self,
        path: &std::path::Path,
    ) -> std::io::Result<Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>>>> {
        self.invoke_callback(path, AccessMode::ReadDir);
        self.which_sys.read_dir(path)
    }

    fn is_valid_executable(&self, path: &std::path::Path) -> std::io::Result<bool> {
        self.invoke_callback(path, AccessMode::Read);
        self.which_sys.is_valid_executable(path)
    }
}

impl<WhichSys, ShebangFS: ShebangParseFileSystem, F: Fn(PathAccess<'_>)> ShebangParseFileSystem
    for SysWithCallback<WhichSys, ShebangFS, F>
{
    type Error = ShebangFS::Error;

    fn peek_executable(
        &self,
        path: &std::path::Path,
        buf: &mut [u8],
    ) -> Result<usize, Self::Error> {
        self.invoke_callback(path, AccessMode::Read);
        self.shebang_fs.peek_executable(path, buf)
    }

    fn shebang_format_error(&self) -> Self::Error {
        self.shebang_fs.shebang_format_error()
    }
}

pub fn real_sys_with_callback(
    cb: impl Fn(PathAccess<'_>),
) -> impl Sys + ShebangParseFileSystem<Error = nix::Error> {
    SysWithCallback {
        which_sys: RealSys::default(),
        shebang_fs: NixFileSystem::default(),
        callback: cb,
    }
}
