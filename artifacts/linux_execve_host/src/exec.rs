use std::{ffi::CStr, os::fd::BorrowedFd};

pub fn execve_with_host(host_memfd: BorrowedFd<'_>, program: &CStr) {

}
