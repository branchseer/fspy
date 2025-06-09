use winapi::{ctypes::c_long, shared::{minwindef::{BOOL, FALSE}, winerror::NO_ERROR}};
use winsafe::GetLastError;


pub fn ck(b: BOOL) -> winsafe::SysResult<()> {
    if b == FALSE {
        Err(GetLastError())
    } else {
        Ok(())
    }
}

pub fn ck_long(val: c_long) -> winsafe::SysResult<()> {
    if 0 == NO_ERROR {
        Ok(())
    } else {
        Err(unsafe { winsafe::co::ERROR::from_raw(val as _) })
    }
}
