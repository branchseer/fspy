mod native_str;

use bincode::{config::Configuration, BorrowDecode, Encode};
pub use native_str::NativeStr;

#[cfg(unix)]
pub use native_str::NativeString;

pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

#[derive(Encode, BorrowDecode, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
    ReadDir,
}

#[derive(Encode, BorrowDecode, Debug, Clone, Copy)]
pub struct PathAccess<'a> {
    pub mode: AccessMode,
    pub path: NativeStr<'a>,
    // pub dir: Option<NativeStr<'a>>,
}
