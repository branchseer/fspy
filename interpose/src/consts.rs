use std::fmt::Debug;

use bincode::{BorrowDecode, Decode, Encode};

#[derive(Encode, Decode, Debug)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Encode, BorrowDecode, Debug)]
pub struct PathAccess<'a> {
    pub access_mode: AccessMode,
    pub path: &'a [u8],
    pub caller: &'a [u8],
}
