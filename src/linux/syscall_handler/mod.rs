use std::{cell::RefCell, ffi::OsStr, io, ops::{Deref, DerefMut}, os::unix::ffi::OsStrExt};

use allocator_api2::vec::Vec;
use fspy_shared::ipc::{AccessMode, NativeStr, PathAccess};
use seccomp_unotify::{handler::arg::{CStrPtr, Fd, Ignored}, impl_handler};
use thread_local::ThreadLocal;
use crate::arena::PathAccessArena;

const PATH_MAX: usize = libc::PATH_MAX as usize;

#[derive(Default, Debug)]
pub struct SyscallHandler {
    pub(crate) arena: PathAccessArena
}

impl SyscallHandler {
    fn openat(&mut self, (_, path): (Ignored, CStrPtr)) -> io::Result<()> {
        path.read_with_buf::<PATH_MAX, _, _>(|path| {
                self.arena.with_accesses_mut(|accesses| {
                // TODO(perf): read path directly into arena-allocated buf
                let path = accesses.allocator().alloc_slice_copy(path);
                let path_access = PathAccess {
                    mode: AccessMode::Read,
                    path: NativeStr::from_bytes(path),
                };
                    accesses.push(path_access);
                });
            Ok(())
        })?;
        Ok(())
    }
    fn getdents64(&mut self, (fd,): (Fd,)) -> io::Result<()> {
        self.arena.with_accesses_mut(|acceeses| {
            let path = acceeses.allocator().alloc_slice_copy(fd.get_path()?.as_bytes());
             let path_access = PathAccess {
                mode: AccessMode::ReadDir,
                path: NativeStr::from_bytes(path),
            };
            acceeses.push(path_access);
            io::Result::Ok(())
        })?;
        // dbg!(fd.get_path())?;
        Ok(())
    }
}

impl_handler!(
    SyscallHandler,
    openat
    getdents64
);
