use std::{cell::Cell, ffi::CStr, slice};

use arrayvec::ArrayVec;
use fspy_shared::ipc::{AccessMode, NativeStr, PathAccess};
use ntapi::ntioapi::{
    FILE_INFORMATION_CLASS, NtQueryFullAttributesFile, NtQueryInformationByName,
    PFILE_BASIC_INFORMATION, PFILE_NETWORK_OPEN_INFORMATION, PIO_STATUS_BLOCK,
};
use smallvec::SmallVec;
use widestring::{U16CStr, U16Str};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, HFILE, MAX_PATH, UINT},
        ntdef::{
            HANDLE, LPCSTR, LPCWSTR, PHANDLE, PLARGE_INTEGER, POBJECT_ATTRIBUTES, PUNICODE_STRING,
            PVOID, ULONG, UNICODE_STRING,
        },
    },
    um::winnt::{ACCESS_MASK, GENERIC_READ, GENERIC_WRITE},
};

use crate::windows::{
    client::global_client,
    detour::{Detour, DetourAny},
    winapi_utils::{access_mask_to_mode, get_path_name, get_u16_str},
};

unsafe fn to_path_access<R, F: FnOnce(PathAccess<'_>) -> R>(
    desired_access: ACCESS_MASK,
    object_attributes: POBJECT_ATTRIBUTES,
    f: F,
) -> R {
    let filename = unsafe { get_u16_str(&*(*object_attributes).ObjectName) };
    let is_absolute = filename.as_slice().first() == Some(&b'\\'.into());

    let dir = if !is_absolute {
        unsafe { get_path_name((*object_attributes).RootDirectory) }.ok()
    } else {
        None
    };
    let dir = if let Some(dir) = &dir {
        Some(NativeStr::from_wide(&dir))
    } else {
        None
    };

    let path_access = PathAccess {
        mode: access_mask_to_mode(desired_access),
        path: NativeStr::from_wide(filename.as_slice()),
        dir,
    };
    f(path_access)
}

thread_local! { pub static IS_DETOURING: Cell<bool> = const { Cell::new(false) }; }

struct DetourGuard {
    active: bool
}

impl DetourGuard {
    pub fn new() -> Self {
        let active = !IS_DETOURING.get();
        if active {
            IS_DETOURING.set(true);
        }
        Self { active }
    }
    pub fn active(&self) -> bool {
        self.active
    }
}

impl Drop for DetourGuard {
    fn drop(&mut self) {
        if self.active {
            IS_DETOURING.set(false);
        }
    }
}

static DETOUR_NT_CREATE_FILE: Detour<
    unsafe extern "system" fn(
        file_handle: PHANDLE,
        desired_access: ACCESS_MASK,
        object_attributes: POBJECT_ATTRIBUTES,
        io_status_block: PIO_STATUS_BLOCK,
        allocation_size: PLARGE_INTEGER,
        file_attributes: ULONG,
        share_access: ULONG,
        create_disposition: ULONG,
        create_options: ULONG,
        ea_buffer: PVOID,
        ea_length: ULONG,
    ) -> HFILE,
> = unsafe {
    Detour::new(ntapi::ntioapi::NtCreateFile, {
        unsafe extern "system" fn new_nt_create_file(
            file_handle: PHANDLE,
            desired_access: ACCESS_MASK,
            object_attributes: POBJECT_ATTRIBUTES,
            io_status_block: PIO_STATUS_BLOCK,
            allocation_size: PLARGE_INTEGER,
            file_attributes: ULONG,
            share_access: ULONG,
            create_disposition: ULONG,
            create_options: ULONG,
            ea_buffer: PVOID,
            ea_length: ULONG,
        ) -> HFILE {
            let guard = DetourGuard::new();
            if guard.active {
                let bt = backtrace::Backtrace::new();
                unsafe {
                    to_path_access(desired_access, object_attributes, |path_access| {
                        eprintln!("NtCreateFile {:?} {:?}", path_access.path, bt);
                        global_client().send(path_access);
                    })
                };
            }

            unsafe {
                (DETOUR_NT_CREATE_FILE.real())(
                    file_handle,
                    desired_access,
                    object_attributes,
                    io_status_block,
                    allocation_size,
                    file_attributes,
                    share_access,
                    create_disposition,
                    create_options,
                    ea_buffer,
                    ea_length,
                )
            }
        }
        new_nt_create_file
    })
};

static DETOUR_NT_OPEN_FILE: Detour<
    unsafe extern "system" fn(
        FileHandle: PHANDLE,
        DesiredAccess: ACCESS_MASK,
        ObjectAttributes: POBJECT_ATTRIBUTES,
        IoStatusBlock: PIO_STATUS_BLOCK,
        ShareAccess: ULONG,
        OpenOptions: ULONG,
    ) -> HFILE,
> = unsafe {
    Detour::new(ntapi::ntioapi::NtOpenFile, {
        unsafe extern "system" fn new_nt_open_file(
            file_handle: PHANDLE,
            desired_access: ACCESS_MASK,
            object_attributes: POBJECT_ATTRIBUTES,
            io_status_block: PIO_STATUS_BLOCK,
            share_access: ULONG,
            open_options: ULONG,
        ) -> HFILE {
            unsafe {
                to_path_access(desired_access, object_attributes, |path_access| {
                    // eprintln!("NtOpenFile {:?}", path_access.path);
                    // global_client().send(path_access);
                })
            };
            unsafe {
                (DETOUR_NT_OPEN_FILE.real())(
                    file_handle,
                    desired_access,
                    object_attributes,
                    io_status_block,
                    share_access,
                    open_options,
                )
            }
        }
        new_nt_open_file
    })
};

static DETOUR_NT_QUERY_ATRRIBUTES_FILE: Detour<
    unsafe extern "system" fn(
        object_attributes: POBJECT_ATTRIBUTES,
        file_information: PFILE_BASIC_INFORMATION,
    ) -> HFILE,
> = unsafe {
    Detour::new(ntapi::ntioapi::NtQueryAttributesFile, {
        unsafe extern "system" fn new_nt_open_file(
            object_attributes: POBJECT_ATTRIBUTES,
            file_information: PFILE_BASIC_INFORMATION,
        ) -> HFILE {
            
            let guard = DetourGuard::new();
            if guard.active {
                unsafe {
                    to_path_access(GENERIC_READ, object_attributes, |path_access| {
                        eprintln!("DETOUR_NT_QUERY_ATRRIBUTES_FILE {:?}", path_access.path);
                        // global_client().send(path_access);
                    })
                };
            }
            unsafe { (DETOUR_NT_QUERY_ATRRIBUTES_FILE.real())(object_attributes, file_information) }
        }
        new_nt_open_file
    })
};

static DETOUR_NT_FULL_QUERY_ATRRIBUTES_FILE: Detour<
    unsafe extern "system" fn(
        object_attributes: POBJECT_ATTRIBUTES,
        file_information: PFILE_NETWORK_OPEN_INFORMATION,
    ) -> HFILE,
> = unsafe {
    Detour::new(NtQueryFullAttributesFile, {
        unsafe extern "system" fn new_fn(
            object_attributes: POBJECT_ATTRIBUTES,
            file_information: PFILE_NETWORK_OPEN_INFORMATION,
        ) -> HFILE {
            
            let guard = DetourGuard::new();
            if guard.active {
            unsafe {
                to_path_access(GENERIC_READ, object_attributes, |path_access| {
                                            eprintln!("NtQueryFullAttributesFile {:?}", path_access.path);

                    // global_client().send(path_access);
                })
            };
        }
            unsafe {
                (DETOUR_NT_FULL_QUERY_ATRRIBUTES_FILE.real())(object_attributes, file_information)
            }
        }
        new_fn
    })
};

static DETOUR_NT_OPEN_SYMBOLIC_LINK_OBJECT: Detour<
    unsafe extern "system" fn(
        link_handle: PHANDLE,
        desired_access: ACCESS_MASK,
        object_attributes: POBJECT_ATTRIBUTES,
    ) -> HFILE,
> = unsafe {
    Detour::new(ntapi::ntobapi::NtOpenSymbolicLinkObject, {
        unsafe extern "system" fn new_fn(
            link_handle: PHANDLE,
            desired_access: ACCESS_MASK,
            object_attributes: POBJECT_ATTRIBUTES,
        ) -> HFILE {
            unsafe {
                to_path_access(desired_access, object_attributes, |path_access| {
                    global_client().send(path_access);
                })
            };
            unsafe {
                (DETOUR_NT_OPEN_SYMBOLIC_LINK_OBJECT.real())(
                    link_handle,
                    desired_access,
                    object_attributes,
                )
            }
        }
        new_fn
    })
};

static DETOUR_NT_QUERY_INFORMATION_BY_NAME: Detour<
    unsafe extern "system" fn(
        object_attributes: POBJECT_ATTRIBUTES,
        io_status_block: PIO_STATUS_BLOCK,
        file_information: PVOID,
        length: ULONG,
        file_information_class: FILE_INFORMATION_CLASS,
    ) -> HFILE,
> = unsafe {
    Detour::new(NtQueryInformationByName, {
        unsafe extern "system" fn new_fn(
            object_attributes: POBJECT_ATTRIBUTES,
            io_status_block: PIO_STATUS_BLOCK,
            file_information: PVOID,
            length: ULONG,
            file_information_class: FILE_INFORMATION_CLASS,
        ) -> HFILE {
            
            let guard = DetourGuard::new();
            if guard.active {

                let bt = backtrace::Backtrace::new();
            unsafe {
                to_path_access(GENERIC_READ, object_attributes, |path_access| {
                    eprintln!("NtQueryInformationByName {:?} {:?}", path_access.path, bt);
                    // global_client().send(path_access);
                })
            };
        }
            unsafe {
                (DETOUR_NT_QUERY_INFORMATION_BY_NAME.real())(
                    object_attributes,
                    io_status_block,
                    file_information,
                    length,
                    file_information_class,
                )
            }
        }
        new_fn
    })
};

#[allow(unused)]
pub const DETOURS: &[DetourAny] = &[
    // DETOUR_NT_CREATE_FILE.as_any(),
    // DETOUR_NT_OPEN_FILE.as_any(),
    DETOUR_NT_QUERY_ATRRIBUTES_FILE.as_any(),
    DETOUR_NT_FULL_QUERY_ATRRIBUTES_FILE.as_any(),
    // DETOUR_NT_OPEN_SYMBOLIC_LINK_OBJECT.as_any(),
    DETOUR_NT_QUERY_INFORMATION_BY_NAME.as_any(),
];
