use std::slice;

use fspy_shared::ipc::AccessMode;
use smallvec::SmallVec;
use widestring::{U16CStr, U16Str};
use winapi::{
    ctypes::c_long,
    shared::{
        minwindef::{BOOL, FALSE, MAX_PATH},
        ntdef::{HANDLE, UNICODE_STRING},
        winerror::NO_ERROR,
    },
    um::{fileapi::GetFinalPathNameByHandleW, winnt::{ACCESS_MASK, GENERIC_READ, GENERIC_WRITE}},
};
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

pub unsafe fn get_u16_str(ustring: &UNICODE_STRING) -> &U16Str {
    let chars =
        unsafe { slice::from_raw_parts((*ustring).Buffer, (*ustring).Length.try_into().unwrap()) };
    match U16CStr::from_slice_truncate(chars) {
        Ok(ok) => ok.as_ustr(),
        Err(_) => chars.into(),
    }
}

pub unsafe fn get_path_name(handle: HANDLE) -> winsafe::SysResult<SmallVec<u16, MAX_PATH>> {
    let mut path = SmallVec::<u16, MAX_PATH>::new();
    let len = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            path.as_mut_ptr(),
            path.capacity().try_into().unwrap(),
            0, /*FILE_NAME_NORMALIZED*/
        )
    };
    if len == 0 {
        return Err(winsafe::GetLastError());
    }
    let len = usize::try_from(len).unwrap();
    if len <= path.capacity() {
        unsafe { path.set_len(len) };
    } else {
        path.reserve_exact(len);
        let len = unsafe {
            GetFinalPathNameByHandleW(
                handle,
                path.as_mut_ptr(),
                path.capacity().try_into().unwrap(),
                0, /*FILE_NAME_NORMALIZED*/
            )
        };
        let len = usize::try_from(len).unwrap();
        if len == 0 {
            return Err(winsafe::GetLastError());
        } else if len > path.capacity() {
            unreachable!()
        }
        unsafe { path.set_len(len) };
    }
    Ok(path)
}



pub fn access_mask_to_mode(desired_access: ACCESS_MASK) -> AccessMode {
    let has_write = (desired_access & GENERIC_WRITE) != 0;
    let has_read = (desired_access & GENERIC_READ) != 0;
    if has_write {
        if has_read {
            AccessMode::ReadWrite
        } else {
            AccessMode::Write
        }
    } else {
        AccessMode::Read
    }
}


#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        fs::File,
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
        path::PathBuf,
    };

    use super::get_path_name;

    fn test_get_path_name(filename: &str) {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().canonicalize().unwrap().join(filename);
        let file = File::create(&path).unwrap();
        let actual_path = unsafe { get_path_name(file.as_raw_handle().cast()) }.unwrap();
        let actual_path = PathBuf::from(OsString::from_wide(&actual_path));
        assert_eq!(path, actual_path);
    }

    #[test]
    fn test_get_path_name_short() {
        test_get_path_name("foo")
    }
    #[test]
    fn test_get_path_name_long() {
        test_get_path_name(str::repeat("a", 255).as_str())
    }
}
