use std::{
    borrow::Borrow as _,
    cell::UnsafeCell,
    ffi::{CStr, c_char, c_long},
    os::raw::c_void,
    str::from_utf8,
};

use arrayvec::ArrayVec;
use ms_detours::{
    DetourAttach, DetourCreateProcessWithDllExW, DetourCreateProcessWithDllsW, DetourDetach,
    DetourFindPayloadEx, DetourIsHelperProcess, DetourRestoreAfterWith, DetourTransactionBegin,
    DetourTransactionCommit, DetourUpdateThread,
};

use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, HINSTANCE, LPVOID, MAX_PATH, TRUE},
        winerror::NO_ERROR,
    },
    um::{
        libloaderapi::GetModuleFileNameA,
        minwinbase::LPSECURITY_ATTRIBUTES,
        processthreadsapi::{
            CreateProcessW as Real_CreateProcessW, GetCurrentThread, LPPROCESS_INFORMATION,
            LPSTARTUPINFOW,
        },
        winnt::{self, LPCWSTR, LPWSTR},
    },
};
// use windows_sys::{
//     Win32::{
//         Foundation::{BOOL, FALSE, HINSTANCE, NO_ERROR, TRUE},
//         Security::SECURITY_ATTRIBUTES,
//         System::{
//             LibraryLoader::GetModuleFileNameA,
//             SystemServices,
//             Threading::{
//                 CreateProcessW as Real_CreateProcessW, GetCurrentThread, PROCESS_CREATION_FLAGS,
//                 PROCESS_INFORMATION, STARTUPINFOW,
//             },
//         },
//     },
//     core::PSTR,
// };
use winsafe::{GetLastError, SetLastError};

struct SyncCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

static DLL_PATH: SyncCell<[u8; MAX_PATH]> = SyncCell(UnsafeCell::new([0; MAX_PATH]));

static CPW: SyncCell<
    unsafe extern "system" fn(
        lpApplicationName: LPCWSTR,
        lpCommandLine: LPWSTR,
        lpProcessAttributes: LPSECURITY_ATTRIBUTES,
        lpThreadAttributes: LPSECURITY_ATTRIBUTES,
        bInheritHandles: BOOL,
        dwCreationFlags: DWORD,
        lpEnvironment: LPVOID,
        lpCurrentDirectory: LPCWSTR,
        lpStartupInfo: LPSTARTUPINFOW,
        lpProcessInformation: LPPROCESS_INFORMATION,
    ) -> BOOL,
> = SyncCell(UnsafeCell::new(Real_CreateProcessW));

unsafe extern "system" fn CreateProcessW(
    lpApplicationName: LPCWSTR,
    lpCommandLine: LPWSTR,
    lpProcessAttributes: LPSECURITY_ATTRIBUTES,
    lpThreadAttributes: LPSECURITY_ATTRIBUTES,
    bInheritHandles: BOOL,
    dwCreationFlags: DWORD,
    lpEnvironment: LPVOID,
    lpCurrentDirectory: LPCWSTR,
    lpStartupInfo: LPSTARTUPINFOW,
    lpProcessInformation: LPPROCESS_INFORMATION,
) -> BOOL {
    eprintln!("CreateProcessW");
    unsafe {
        DetourCreateProcessWithDllExW(
            lpApplicationName,
            lpCommandLine,
            lpProcessAttributes,
            lpThreadAttributes,
            bInheritHandles,
            dwCreationFlags,
            lpEnvironment,
            lpCurrentDirectory,
            lpStartupInfo,
            lpProcessInformation,
            (*DLL_PATH.0.get()).as_ptr().cast(),
            Some(*CPW.0.get().cast()),
        )
        // ;(*CPW.0.get())(
        //     lpapplicationname,
        //     lpcommandline,
        //     lpprocessattributes,
        //     lpthreadattributes,
        //     binherithandles,
        //     dwcreationflags,
        //     lpenvironment,
        //     lpcurrentdirectory,
        //     lpstartupinfo,
        //     lpprocessinformation,
        // )
    }
}

fn ck(b: BOOL) -> winsafe::SysResult<()> {
    if b == FALSE {
        Err(GetLastError())
    } else {
        Ok(())
    }
}

fn ck_long(val: c_long) -> winsafe::SysResult<()> {
    if 0 == NO_ERROR {
        Ok(())
    } else {
        Err(unsafe { winsafe::co::ERROR::from_raw(val as _) })
    }
}

fn dll_main(hinstance: HINSTANCE, reason: u32) -> winsafe::SysResult<()> {
    if unsafe { DetourIsHelperProcess() } == TRUE {
        return Ok(());
    }

    let cpw = CPW.0.get() as _;
    match reason {
        winnt::DLL_PROCESS_ATTACH => {
            unsafe {
                GetModuleFileNameA(
                    hinstance,
                    DLL_PATH
                        .0
                        .get()
                        .as_mut()
                        .unwrap_unchecked()
                        .as_mut_ptr()
                        .cast(),
                    MAX_PATH.try_into().unwrap(),
                )
            };
            ck(unsafe { DetourRestoreAfterWith() })?;

            ck_long(unsafe { DetourTransactionBegin() })?;
            ck_long(unsafe { DetourUpdateThread(GetCurrentThread().cast()) })?;

            ck_long(unsafe { DetourAttach(cpw, CreateProcessW as _) })?;

            ck_long(unsafe { DetourTransactionCommit() })?;
        }
        winnt::DLL_PROCESS_DETACH => {
            ck(unsafe { DetourTransactionBegin() })?;
            ck(unsafe { DetourUpdateThread(GetCurrentThread().cast()) })?;

            ck_long(unsafe { DetourDetach(cpw, CreateProcessW as _) })?;

            ck(unsafe { DetourTransactionCommit() })?;
        }
        _ => {}
    }
    Ok(())
}

#[unsafe(no_mangle)]
#[allow(non_snake_case, unused_variables)]
extern "system" fn DllMain(hinstance: HINSTANCE, reason: u32, _: *mut std::ffi::c_void) -> BOOL {
    match dll_main(hinstance, reason) {
        Ok(()) => TRUE,
        Err(err) => {
            SetLastError(err);
            FALSE
        }
    }
}
