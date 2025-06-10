use std::{ffi::CStr, slice};

use arrayvec::ArrayVec;
use fspy_shared::ipc::{AccessMode, NativeStr, PathAccess};
use smallvec::SmallVec;
use widestring::{U16CStr, U16Str};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, HFILE, LPVOID, MAX_PATH, UINT},
        ntdef::{
            HANDLE, LPCSTR, LPCWSTR, PHANDLE, PLARGE_INTEGER, POBJECT_ATTRIBUTES, PUNICODE_STRING,
            PVOID, ULONG, UNICODE_STRING,
        },
    },
    um::{
        fileapi::{
            CreateFileA, CreateFileW, GetFileAttributesA, GetFileAttributesExA,
            GetFileAttributesExW, GetFileAttributesW, GetFinalPathNameByHandleW,
        },
        minwinbase::{GET_FILEEX_INFO_LEVELS, LPSECURITY_ATTRIBUTES},
        winbase::{
            CreateFileMappingA, GetFileAttributesTransactedA, GetFileAttributesTransactedW,
            LPOFSTRUCT, OpenFile, OpenFileMappingA,
        },
        winnt::{ACCESS_MASK, GENERIC_READ, GENERIC_WRITE},
    },
};

use crate::windows::{
    client::global_client,
    detour::{Detour, DetourAny},
    winapi_utils::{access_mask_to_mode, get_path_name, get_u16_str},
};

static DETOUR_CREATE_FILE_W: Detour<
    unsafe extern "system" fn(
        lpFileName: LPCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE,
> = unsafe {
    Detour::new(CreateFileW, {
        unsafe extern "system" fn new_create_file_w(
            lp_file_name: LPCWSTR,
            dw_desired_access: DWORD,
            dw_share_mode: DWORD,
            lp_security_attributes: LPSECURITY_ATTRIBUTES,
            dw_creation_disposition: DWORD,
            dw_flags_and_attributes: DWORD,
            h_template_file: HANDLE,
        ) -> HANDLE {
            let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
            
            if std::env::current_exe().unwrap().ends_with("node.exe") {
                dbg!(filename);
            }
            unsafe { global_client() }.send(PathAccess {
                mode: access_mask_to_mode(dw_desired_access),
                path: NativeStr::from_wide(filename.as_slice()),
                dir: None,
            });
            unsafe {
                (DETOUR_CREATE_FILE_W.real())(
                    lp_file_name,
                    dw_desired_access,
                    dw_share_mode,
                    lp_security_attributes,
                    dw_creation_disposition,
                    dw_flags_and_attributes,
                    h_template_file,
                )
            }
        }
        new_create_file_w
    })
};

static DETOUR_CREATE_FILE_A: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        dw_desired_access: DWORD,
        dw_share_mode: DWORD,
        lp_security_attributes: LPSECURITY_ATTRIBUTES,
        dw_creation_disposition: DWORD,
        dw_flags_and_attributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE,
> = unsafe {
    Detour::new(CreateFileA, {
        unsafe extern "system" fn new_create_file_a(
            lp_file_name: LPCSTR,
            dw_desired_access: DWORD,
            dw_share_mode: DWORD,
            lp_security_attributes: LPSECURITY_ATTRIBUTES,
            dw_creation_disposition: DWORD,
            dw_flags_and_attributes: DWORD,
            h_template_file: HANDLE,
        ) -> HANDLE {
            let filename = unsafe { CStr::from_ptr(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: access_mask_to_mode(dw_desired_access),
                path: NativeStr::from_bytes(filename.to_bytes()),
                dir: None,
            });
            unsafe {
                (DETOUR_CREATE_FILE_A.real())(
                    lp_file_name,
                    dw_desired_access,
                    dw_share_mode,
                    lp_security_attributes,
                    dw_creation_disposition,
                    dw_flags_and_attributes,
                    h_template_file,
                )
            }
        }
        new_create_file_a
    })
};

static DETOUR_OPEN_FILE: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        lp_re_open_buff: LPOFSTRUCT,
        u_style: UINT,
    ) -> HFILE,
> = unsafe {
    Detour::new(OpenFile, {
        unsafe extern "system" fn new_open_file(
            lp_file_name: LPCSTR,
            lp_re_open_buff: LPOFSTRUCT,
            u_style: UINT,
        ) -> HFILE {
            let filename = unsafe { CStr::from_ptr(lp_file_name) };

            // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-openfile
            const OF_WRITE: UINT = 0x00000001;
            const OF_READWRITE: UINT = 0x00000002;
            const OF_DELETE: UINT = 0x00000200;
            const OF_CREATE: UINT = 0x00001000;

            unsafe { global_client() }.send(PathAccess {
                mode: if u_style & OF_READWRITE != 0 {
                    AccessMode::ReadWrite
                } else if u_style & OF_WRITE != 0
                    || u_style & OF_DELETE != 0
                    || u_style & OF_CREATE != 0
                {
                    AccessMode::Write
                } else {
                    AccessMode::Read
                },
                path: NativeStr::from_bytes(filename.to_bytes()),
                dir: None,
            });

            unsafe { (DETOUR_OPEN_FILE.real())(lp_file_name, lp_re_open_buff, u_style) }
        }
        new_open_file
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_W: Detour<
    unsafe extern "system" fn(lp_file_name: LPCWSTR) -> DWORD,
> = unsafe {
    Detour::new(GetFileAttributesW, {
        unsafe extern "system" fn new_fn(lp_file_name: LPCWSTR) -> DWORD {
            let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_wide(filename.as_slice()),
                dir: None,
            });

            unsafe { (DETOUR_GET_FILE_ATTRIBUTES_W.real())(lp_file_name) }
        }
        new_fn
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_A: Detour<
    unsafe extern "system" fn(lp_file_name: LPCSTR) -> DWORD,
> = unsafe {
    Detour::new(GetFileAttributesA, {
        unsafe extern "system" fn new_fn(lp_file_name: LPCSTR) -> DWORD {
            let filename = unsafe { CStr::from_ptr(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_bytes(filename.to_bytes()),
                dir: None,
            });

            unsafe { (DETOUR_GET_FILE_ATTRIBUTES_A.real())(lp_file_name) }
        }
        new_fn
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_EX_W: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCWSTR,
        f_info_level_id: GET_FILEEX_INFO_LEVELS,
        lp_file_information: LPVOID,
    ) -> BOOL,
> = unsafe {
    Detour::new(GetFileAttributesExW, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCWSTR,
            f_info_level_id: GET_FILEEX_INFO_LEVELS,
            lp_file_information: LPVOID,
        ) -> BOOL {
            let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_wide(filename.as_slice()),
                dir: None,
            });

            unsafe {
                (DETOUR_GET_FILE_ATTRIBUTES_EX_W.real())(
                    lp_file_name,
                    f_info_level_id,
                    lp_file_information,
                )
            }
        }
        new_fn
    })
};
static DETOUR_GET_FILE_ATTRIBUTES_EX_A: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        f_info_level_id: GET_FILEEX_INFO_LEVELS,
        lp_file_information: LPVOID,
    ) -> BOOL,
> = unsafe {
    Detour::new(GetFileAttributesExA, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCSTR,
            f_info_level_id: GET_FILEEX_INFO_LEVELS,
            lp_file_information: LPVOID,
        ) -> BOOL {
            let filename = unsafe { CStr::from_ptr(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_bytes(filename.to_bytes()),
                dir: None,
            });
            unsafe {
                (DETOUR_GET_FILE_ATTRIBUTES_EX_A.real())(
                    lp_file_name,
                    f_info_level_id,
                    lp_file_information,
                )
            }
        }
        new_fn
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_W: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCWSTR,
        f_info_level_id: GET_FILEEX_INFO_LEVELS,
        lp_file_information: LPVOID,
        h_transaction: HANDLE,
    ) -> BOOL,
> = unsafe {
    Detour::new(GetFileAttributesTransactedW, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCWSTR,
            f_info_level_id: GET_FILEEX_INFO_LEVELS,
            lp_file_information: LPVOID,
            h_transaction: HANDLE,
        ) -> BOOL {
            let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_wide(filename.as_slice()),
                dir: None,
            });

            unsafe {
                (DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_W.real())(
                    lp_file_name,
                    f_info_level_id,
                    lp_file_information,
                    h_transaction,
                )
            }
        }
        new_fn
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_A: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        f_info_level_id: GET_FILEEX_INFO_LEVELS,
        lp_file_information: LPVOID,
        h_transaction: HANDLE,
    ) -> BOOL,
> = unsafe {
    Detour::new(GetFileAttributesTransactedA, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCSTR,
            f_info_level_id: GET_FILEEX_INFO_LEVELS,
            lp_file_information: LPVOID,
            h_transaction: HANDLE,
        ) -> BOOL {
            let filename = unsafe { CStr::from_ptr(lp_file_name) };
            unsafe { global_client() }.send(PathAccess {
                mode: AccessMode::Read,
                path: NativeStr::from_bytes(filename.to_bytes()),
                dir: None,
            });

            unsafe {
                (DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_A.real())(
                    lp_file_name,
                    f_info_level_id,
                    lp_file_information,
                    h_transaction,
                )
            }
        }
        new_fn
    })
};

pub const DETOURS: &[DetourAny] = &[
    DETOUR_CREATE_FILE_W.as_any(),
    DETOUR_CREATE_FILE_A.as_any(),
    DETOUR_OPEN_FILE.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_W.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_A.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_EX_W.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_EX_A.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_W.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_A.as_any(),
];
