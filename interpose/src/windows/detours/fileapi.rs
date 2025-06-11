use std::ffi::{CStr, c_int};

use fspy_shared::ipc::{AccessMode, NativeStr, PathAccess};

use widestring::U16CStr;
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, HFILE, LPVOID, UINT},
        ntdef::{
            HANDLE, LPCSTR, LPCWSTR, PCWSTR, PHANDLE, PLARGE_INTEGER, POBJECT_ATTRIBUTES,
            PUNICODE_STRING, PVOID, ULONG, UNICODE_STRING,
        },
    },
    um::{
        fileapi::{
            CreateFile2, CreateFileA, DeleteFileA, DeleteFileW, FindFirstFileA, FindFirstFileExA,
            FindFirstFileExW, FindFirstFileW, GetFileAttributesA, GetFileAttributesExA,
            GetFileAttributesExW, GetFileAttributesW, LPCREATEFILE2_EXTENDED_PARAMETERS,
        },
        minwinbase::{
            FINDEX_INFO_LEVELS, FINDEX_SEARCH_OPS, GET_FILEEX_INFO_LEVELS, LPSECURITY_ATTRIBUTES,
            LPWIN32_FIND_DATAA, LPWIN32_FIND_DATAW,
        },
        winbase::{
            GetFileAttributesTransactedA, GetFileAttributesTransactedW, LPOFSTRUCT, OpenFile,
        },
    },
};

use crate::windows::{
    client::global_client,
    detour::{Detour, DetourAny},
    winapi_utils::access_mask_to_mode,
};

unsafe extern "system" {
    unsafe fn CreateFileW(
        lp_file_name: LPCWSTR,
        dw_desired_access: DWORD,
        dw_share_mode: DWORD,
        lp_security_attributes: LPSECURITY_ATTRIBUTES,
        dw_creation_disposition: DWORD,
        dw_flags_and_attributes: DWORD,
        h_template_file: HANDLE,
    ) -> HANDLE;
}

static DETOUR_CREATE_FILE_W: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCWSTR,
        dw_desired_access: DWORD,
        dw_share_mode: DWORD,
        lp_security_attributes: LPSECURITY_ATTRIBUTES,
        dw_creation_disposition: DWORD,
        dw_flags_and_attributes: DWORD,
        h_template_file: HANDLE,
    ) -> HANDLE,
> = unsafe {
    Detour::new(c"CreateFileW", CreateFileW, {
        unsafe extern "system" fn new_create_file_w(
            lp_file_name: LPCWSTR,
            dw_desired_access: DWORD,
            dw_share_mode: DWORD,
            lp_security_attributes: LPSECURITY_ATTRIBUTES,
            dw_creation_disposition: DWORD,
            dw_flags_and_attributes: DWORD,
            h_template_file: HANDLE,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: access_mask_to_mode(dw_desired_access),
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    })
                }
            }
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
    Detour::new(c"CreateFileA", CreateFileA, {
        unsafe extern "system" fn new_create_file_a(
            lp_file_name: LPCSTR,
            dw_desired_access: DWORD,
            dw_share_mode: DWORD,
            lp_security_attributes: LPSECURITY_ATTRIBUTES,
            dw_creation_disposition: DWORD,
            dw_flags_and_attributes: DWORD,
            h_template_file: HANDLE,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: access_mask_to_mode(dw_desired_access),
                        path: NativeStr::from_bytes(filename.to_bytes()),
                        dir: None,
                    })
                };
            }
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

static DETOUR_CREATE_FILE_2: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCWSTR,
        dw_desired_access: DWORD,
        dw_share_mode: DWORD,
        dw_creation_disposition: DWORD,
        p_create_ex_params: LPCREATEFILE2_EXTENDED_PARAMETERS,
    ) -> HANDLE,
> = unsafe {
    Detour::new(c"CreateFile2", CreateFile2, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCWSTR,
            dw_desired_access: DWORD,
            dw_share_mode: DWORD,
            dw_creation_disposition: DWORD,
            p_create_ex_params: LPCREATEFILE2_EXTENDED_PARAMETERS,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: access_mask_to_mode(dw_desired_access),
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    })
                };
            }
            unsafe {
                (DETOUR_CREATE_FILE_2.real())(
                    lp_file_name,
                    dw_desired_access,
                    dw_share_mode,
                    dw_creation_disposition,
                    p_create_ex_params,
                )
            }
        }
        new_fn
    })
};

static DETOUR_OPEN_FILE: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        lp_re_open_buff: LPOFSTRUCT,
        u_style: UINT,
    ) -> HFILE,
> = unsafe {
    Detour::new(c"OpenFile", OpenFile, {
        unsafe extern "system" fn new_open_file(
            lp_file_name: LPCSTR,
            lp_re_open_buff: LPOFSTRUCT,
            u_style: UINT,
        ) -> HFILE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };

                // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-openfile
                const OF_WRITE: UINT = 0x00000001;
                const OF_READWRITE: UINT = 0x00000002;
                const OF_DELETE: UINT = 0x00000200;
                const OF_CREATE: UINT = 0x00001000;

                unsafe {
                    sender.send(PathAccess {
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
                    })
                };
            }

            unsafe { (DETOUR_OPEN_FILE.real())(lp_file_name, lp_re_open_buff, u_style) }
        }
        new_open_file
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_W: Detour<
    unsafe extern "system" fn(lp_file_name: LPCWSTR) -> DWORD,
> = unsafe {
    Detour::new(c"GetFileAttributesW", GetFileAttributesW, {
        unsafe extern "system" fn new_fn(lp_file_name: LPCWSTR) -> DWORD {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    })
                };
            }

            unsafe { (DETOUR_GET_FILE_ATTRIBUTES_W.real())(lp_file_name) }
        }
        new_fn
    })
};

static DETOUR_GET_FILE_ATTRIBUTES_A: Detour<
    unsafe extern "system" fn(lp_file_name: LPCSTR) -> DWORD,
> = unsafe {
    Detour::new(c"GetFileAttributesA", GetFileAttributesA, {
        unsafe extern "system" fn new_fn(lp_file_name: LPCSTR) -> DWORD {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_bytes(filename.to_bytes()),
                        dir: None,
                    });
                }
            }

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
    Detour::new(c"GetFileAttributesExW", GetFileAttributesExW, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCWSTR,
            f_info_level_id: GET_FILEEX_INFO_LEVELS,
            lp_file_information: LPVOID,
        ) -> BOOL {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    });
                }
            }

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
    Detour::new(c"GetFileAttributesExA", GetFileAttributesExA, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCSTR,
            f_info_level_id: GET_FILEEX_INFO_LEVELS,
            lp_file_information: LPVOID,
        ) -> BOOL {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_bytes(filename.to_bytes()),
                        dir: None,
                    });
                }
            }
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
    Detour::new(
        c"GetFileAttributesTransactedW",
        GetFileAttributesTransactedW,
        {
            unsafe extern "system" fn new_fn(
                lp_file_name: LPCWSTR,
                f_info_level_id: GET_FILEEX_INFO_LEVELS,
                lp_file_information: LPVOID,
                h_transaction: HANDLE,
            ) -> BOOL {
                let sender = unsafe { global_client() }.sender();
                if let Some(sender) = &sender {
                    let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                    unsafe {
                        sender.send(PathAccess {
                            mode: AccessMode::Read,
                            path: NativeStr::from_wide(filename.as_slice()),
                            dir: None,
                        });
                    }
                }

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
        },
    )
};

static DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_A: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        f_info_level_id: GET_FILEEX_INFO_LEVELS,
        lp_file_information: LPVOID,
        h_transaction: HANDLE,
    ) -> BOOL,
> = unsafe {
    Detour::new(
        c"GetFileAttributesTransactedA",
        GetFileAttributesTransactedA,
        {
            unsafe extern "system" fn new_fn(
                lp_file_name: LPCSTR,
                f_info_level_id: GET_FILEEX_INFO_LEVELS,
                lp_file_information: LPVOID,
                h_transaction: HANDLE,
            ) -> BOOL {
                let sender = unsafe { global_client() }.sender();
                if let Some(sender) = &sender {
                    let filename = unsafe { CStr::from_ptr(lp_file_name) };
                    unsafe {
                        sender.send(PathAccess {
                            mode: AccessMode::Read,
                            path: NativeStr::from_bytes(filename.to_bytes()),
                            dir: None,
                        });
                    }
                }

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
        },
    )
};

static DETOUR_DELETE_FILE_W: Detour<unsafe extern "system" fn(lp_file_name: LPCWSTR) -> BOOL> = unsafe {
    Detour::new(c"DeleteFileW", DeleteFileW, {
        unsafe extern "system" fn new_fn(lp_file_name: LPCWSTR) -> BOOL {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Write,
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    });
                }
            }

            unsafe { (DETOUR_DELETE_FILE_W.real())(lp_file_name) }
        }
        new_fn
    })
};
static DETOUR_DELETE_FILE_A: Detour<unsafe extern "system" fn(lp_file_name: LPCSTR) -> BOOL> = unsafe {
    Detour::new(c"DeleteFileA", DeleteFileA, {
        unsafe extern "system" fn new_fn(lp_file_name: LPCSTR) -> BOOL {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Write,
                        path: NativeStr::from_bytes(filename.to_bytes()),
                        dir: None,
                    });
                }
            }

            unsafe { (DETOUR_DELETE_FILE_A.real())(lp_file_name) }
        }
        new_fn
    })
};

static DETOUR_FIND_FIRST_FILE_W: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCWSTR,
        lp_find_file_data: LPWIN32_FIND_DATAW,
    ) -> HANDLE,
> = unsafe {
    Detour::new(c"FindFirstFileW", FindFirstFileW, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCWSTR,
            lp_find_file_data: LPWIN32_FIND_DATAW,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    });
                }
            }

            unsafe { (DETOUR_FIND_FIRST_FILE_W.real())(lp_file_name, lp_find_file_data) }
        }
        new_fn
    })
};

static DETOUR_FIND_FIRST_FILE_EX_W: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCWSTR,
        f_info_level_id: FINDEX_INFO_LEVELS,
        lp_find_file_data: LPVOID,
        f_search_op: FINDEX_SEARCH_OPS,
        lp_search_filter: LPVOID,
        dw_additional_flags: DWORD,
    ) -> HANDLE,
> = unsafe {
    Detour::new(c"FindFirstFileExW", FindFirstFileExW, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCWSTR,
            f_info_level_id: FINDEX_INFO_LEVELS,
            lp_find_file_data: LPVOID,
            f_search_op: FINDEX_SEARCH_OPS,
            lp_search_filter: LPVOID,
            dw_additional_flags: DWORD,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    });
                }
            }

            unsafe {
                (DETOUR_FIND_FIRST_FILE_EX_W.real())(
                    lp_file_name,
                    f_info_level_id,
                    lp_find_file_data,
                    f_search_op,
                    lp_search_filter,
                    dw_additional_flags,
                )
            }
        }
        new_fn
    })
};

static DETOUR_FIND_FIRST_FILE_A: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        lp_find_file_data: LPWIN32_FIND_DATAA,
    ) -> HANDLE,
> = unsafe {
    Detour::new(c"FindFirstFileA", FindFirstFileA, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCSTR,
            lp_find_file_data: LPWIN32_FIND_DATAA,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_bytes(filename.to_bytes()),
                        dir: None,
                    });
                }
            }
            unsafe { (DETOUR_FIND_FIRST_FILE_A.real())(lp_file_name, lp_find_file_data) }
        }
        new_fn
    })
};

static DETOUR_FIND_FIRST_FILE_EX_A: Detour<
    unsafe extern "system" fn(
        lp_file_name: LPCSTR,
        f_info_level_id: FINDEX_INFO_LEVELS,
        lp_find_file_data: LPVOID,
        f_search_op: FINDEX_SEARCH_OPS,
        lp_search_filter: LPVOID,
        dw_additional_flags: DWORD,
    ) -> HANDLE,
> = unsafe {
    Detour::new(c"FindFirstFileExA", FindFirstFileExA, {
        unsafe extern "system" fn new_fn(
            lp_file_name: LPCSTR,
            f_info_level_id: FINDEX_INFO_LEVELS,
            lp_find_file_data: LPVOID,
            f_search_op: FINDEX_SEARCH_OPS,
            lp_search_filter: LPVOID,
            dw_additional_flags: DWORD,
        ) -> HANDLE {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { CStr::from_ptr(lp_file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_bytes(filename.to_bytes()),
                        dir: None,
                    });
                }
            }

            unsafe {
                (DETOUR_FIND_FIRST_FILE_EX_A.real())(
                    lp_file_name,
                    f_info_level_id,
                    lp_find_file_data,
                    f_search_op,
                    lp_search_filter,
                    dw_additional_flags,
                )
            }
        }
        new_fn
    })
};

// Added in Windows 11 24h2.
// Used in libuv: https://github.com/libuv/libuv/blob/b00c5d1a09c094020044e79e19f478a25b8e1431/src/win/winapi.c#L142
static DETOUR_GET_FILE_INFORMATION_BY_NAME: Detour<
    unsafe extern "system" fn(
        file_name: PCWSTR,
        file_information_class: c_int, // FILE_INFO_BY_NAME_CLASS,
        file_info_buffer: PVOID,
        file_info_buffer_size: ULONG,
    ) -> BOOL,
> = unsafe {
    Detour::dynamic(c"GetFileInformationByName", {
        unsafe extern "system" fn new_fn(
            file_name: PCWSTR,
            file_information_class: c_int, // FILE_INFO_BY_NAME_CLASS,
            file_info_buffer: PVOID,
            file_info_buffer_size: ULONG,
        ) -> BOOL {
            let sender = unsafe { global_client() }.sender();
            if let Some(sender) = &sender {
                let filename = unsafe { U16CStr::from_ptr_str(file_name) };
                unsafe {
                    sender.send(PathAccess {
                        mode: AccessMode::Read,
                        path: NativeStr::from_wide(filename.as_slice()),
                        dir: None,
                    });
                }
            }

            unsafe {
                (DETOUR_GET_FILE_INFORMATION_BY_NAME.real())(
                    file_name,
                    file_information_class,
                    file_info_buffer,
                    file_info_buffer_size,
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
    DETOUR_CREATE_FILE_2.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_W.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_A.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_EX_W.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_EX_A.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_W.as_any(),
    DETOUR_GET_FILE_ATTRIBUTES_TRANSACTED_A.as_any(),
    DETOUR_DELETE_FILE_W.as_any(),
    DETOUR_DELETE_FILE_A.as_any(),
    DETOUR_FIND_FIRST_FILE_W.as_any(),
    DETOUR_FIND_FIRST_FILE_EX_W.as_any(),
    DETOUR_FIND_FIRST_FILE_A.as_any(),
    DETOUR_FIND_FIRST_FILE_EX_A.as_any(),
    DETOUR_GET_FILE_INFORMATION_BY_NAME.as_any(),
];
