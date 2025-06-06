use std::{
    borrow::Borrow as _, cell::UnsafeCell, ffi::{c_char, c_long, CStr}, os::raw::c_void, str::from_utf8
};

use arrayvec::ArrayVec;
use ms_detours::{
    DetourAttach, DetourCreateProcessWithDllExW, DetourCreateProcessWithDllsW, DetourDetach,
    DetourIsHelperProcess, DetourRestoreAfterWith, DetourTransactionBegin, DetourTransactionCommit,
    DetourUpdateThread,
};
use windows_sys::{
    Win32::{
        Foundation::{BOOL, FALSE, HINSTANCE, NO_ERROR, TRUE},
        Security::SECURITY_ATTRIBUTES,
        System::{
            LibraryLoader::GetModuleFileNameA,
            SystemServices,
            Threading::{
                CreateProcessW as Real_CreateProcessW, GetCurrentThread, PROCESS_CREATION_FLAGS,
                PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
    },
    core::PSTR,
};
use winsafe::{GetLastError, SetLastError};

struct SyncCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

const MAX_PATH: usize = windows_sys::Win32::Foundation::MAX_PATH as usize;

static DLL_PATH: SyncCell<[u8; MAX_PATH]> = SyncCell(UnsafeCell::new([0; MAX_PATH]));

static CPW: SyncCell<
    unsafe extern "system" fn(
        lpapplicationname: windows_sys::core::PCWSTR,
        lpcommandline: windows_sys::core::PWSTR,
        lpprocessattributes: *const SECURITY_ATTRIBUTES,
        lpthreadattributes: *const SECURITY_ATTRIBUTES,
        binherithandles: BOOL,
        dwcreationflags: PROCESS_CREATION_FLAGS,
        lpenvironment: *const core::ffi::c_void,
        lpcurrentdirectory: windows_sys::core::PCWSTR,
        lpstartupinfo: *const STARTUPINFOW,
        lpprocessinformation: *mut PROCESS_INFORMATION,
    ) -> BOOL,
> = SyncCell(UnsafeCell::new(Real_CreateProcessW));

unsafe extern "system" fn CreateProcessW(
    lpapplicationname: windows_sys::core::PCWSTR,
    lpcommandline: windows_sys::core::PWSTR,
    lpprocessattributes: *const SECURITY_ATTRIBUTES,
    lpthreadattributes: *const SECURITY_ATTRIBUTES,
    binherithandles: BOOL,
    dwcreationflags: PROCESS_CREATION_FLAGS,
    lpenvironment: *const core::ffi::c_void,
    lpcurrentdirectory: windows_sys::core::PCWSTR,
    lpstartupinfo: *const STARTUPINFOW,
    lpprocessinformation: *mut PROCESS_INFORMATION,
) -> BOOL {
    eprintln!("CreateProcessW");
    unsafe {
        DetourCreateProcessWithDllExW(
            lpapplicationname.cast(),
            lpcommandline.cast(),
            lpprocessattributes.cast_mut().cast(),
            lpthreadattributes.cast_mut().cast(),
            binherithandles,
            dwcreationflags,
            lpenvironment.cast_mut().cast(),
            lpcurrentdirectory.cast(),
            lpstartupinfo.cast_mut().cast(),
            lpprocessinformation.cast(),
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
        SystemServices::DLL_PROCESS_ATTACH => {
            unsafe {
                GetModuleFileNameA(
                    hinstance,
                    DLL_PATH.0.get().as_mut().unwrap_unchecked().as_mut_ptr(),
                    windows_sys::Win32::Foundation::MAX_PATH,
                )
            };
            ck(unsafe { DetourRestoreAfterWith() })?;

            ck_long(unsafe { DetourTransactionBegin() })?;
            ck_long(unsafe { DetourUpdateThread(GetCurrentThread().cast()) })?;

            ck_long(unsafe { DetourAttach(cpw, CreateProcessW as _) })?;

            ck_long(unsafe { DetourTransactionCommit() })?;
        }
        SystemServices::DLL_PROCESS_DETACH => {
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
