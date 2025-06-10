use std::{cell::SyncUnsafeCell, os::raw::c_void};

use ms_detours::{DetourAttach, DetourDetach};
use winsafe::SysResult;

use crate::windows::winapi_utils::ck_long;

pub struct Detour<T> {
    real: SyncUnsafeCell<T>,
    new: T,
}

// struct Detour<T>(DetourPair, PhantomData<*mut T>);
impl<T> Detour<T> {
    pub const unsafe fn new(real: T, new: T) -> Self {
        Detour {
            real: SyncUnsafeCell::new(real),
            new: new,
        }
    }
    pub fn real(&self) -> &T {
        unsafe { &*self.real.get() }
    }
    pub const fn as_any(&'static self) -> DetourAny
    where
        T: Copy,
    {
        DetourAny {
            real: self.real.get().cast(),
            new: (&self.new as *const T).cast(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct DetourAny {
    real: *mut *mut c_void,
    new: *const *mut c_void,
}

unsafe impl Sync for DetourAny {}
impl DetourAny {
    pub unsafe fn attach(&self) -> SysResult<()> {
        ck_long(unsafe { DetourAttach(self.real.cast(), (*self.new).cast()) })
    }
    pub unsafe fn detach(&self) -> SysResult<()> {
        ck_long(unsafe { DetourDetach(self.real.cast(), (*self.new).cast()) })
    }
}
