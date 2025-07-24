use std::{cell::RefCell, ffi::OsStr, io, ops::{Deref, DerefMut}, os::unix::ffi::OsStrExt};

use allocator_api2::vec::Vec;
use fspy_shared::ipc::{AccessMode, NativeStr, PathAccess};
use seccomp_unotify::{handler::{arg::{Ignored, CStrPtr}}, impl_handler};
use thread_local::ThreadLocal;
use crate::arena::PathAccessArena;

const PATH_MAX: usize = libc::PATH_MAX as usize;

#[derive(Default, Debug)]
pub struct SyscallHandler {
    pub(crate) arena: PathAccessArena
}

impl SyscallHandler {
    fn openat(&mut self, (_, path): (Ignored, CStrPtr)) -> io::Result<()> {
        // let mut arena = self.tls_arena.get_or_default().borrow_mut();
        // let arena = arena.deref_mut();
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
}

impl_handler!(
    SyscallHandler,
    openat
);
