mod native_str;

use bincode::{config::Configuration, BorrowDecode, Encode};
pub use native_str::NativeStr;

#[cfg(unix)]
pub use native_str::NativeString;

pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

#[derive(Encode, BorrowDecode, Debug)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Encode, BorrowDecode, Debug)]
pub struct PathAccess<'a> {
    pub mode: AccessMode,
    pub path: NativeStr<'a>,
    pub dir: Option<NativeStr<'a>>,
}
