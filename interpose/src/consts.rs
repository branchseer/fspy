use std::fmt::Debug;

use bincode::{BorrowDecode, Decode, Encode};

#[derive(Encode, Decode, Debug)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Encode, BorrowDecode)]
pub struct PathAccess<'a> {
    pub access_mode: AccessMode,
    pub path: &'a [u8],
    pub caller: &'a [u8],
}

impl<'a> Debug for PathAccess<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};
        f.debug_struct("PathAccess")
            .field("access_mode", &self.access_mode)
            .field("path",  &OsStr::from_bytes(self.path))
            .field("caller", &OsStr::from_bytes(self.caller))
            .finish()
    }
}
