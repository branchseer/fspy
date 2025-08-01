use which::sys::{RealSys, Sys};

use crate::shebang;

pub struct RealHandleSpawnFileSystem {
    shebang_fs: shebang::NixFileSystem,
    which_sys: RealSys,
}


impl shebang::FileSystem for RealHandleSpawnFileSystem {
    type Error = nix::Error;

    fn peek_executable(
        &self,
        path: &std::path::Path,
        buf: &mut [u8],
    ) -> Result<usize, Self::Error> {
        self.shebang_fs.peek_executable(path, buf)
    }

    fn format_error(&self) -> Self::Error {
        self.shebang_fs.format_error()
    }
}

impl Sys for RealHandleSpawnFileSystem {
    type ReadDirEntry = <RealSys as Sys>::ReadDirEntry;

    type Metadata = <RealSys as Sys>::Metadata;

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
        self.which_sys.metadata(path)
    }

    fn symlink_metadata(&self, path: &std::path::Path) -> std::io::Result<Self::Metadata> {
        self.which_sys.symlink_metadata(path)
    }

    fn read_dir(
        &self,
        path: &std::path::Path,
    ) -> std::io::Result<Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>>>> {
        self.which_sys.read_dir(path)
    }

    fn is_valid_executable(&self, path: &std::path::Path) -> std::io::Result<bool> {
        self.which_sys.is_valid_executable(path)
    }
}
