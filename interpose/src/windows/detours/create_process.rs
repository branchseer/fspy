use crate::windows::{
    client::global_client,
    detour::{Detour, DetourAny},
};

use fspy_shared::{
    ipc::{AccessMode, NativeStr, PathAccess},
    windows::PAYLOAD_ID,
};
use ms_detours::{DetourCopyPayloadToProcess, DetourCreateProcessWithDllExW};
use widestring::U16CStr;
use winapi::{
    shared::minwindef::{BOOL, DWORD, LPVOID},
    um::{
        minwinbase::LPSECURITY_ATTRIBUTES,
        processthreadsapi::{
            CreateProcessW as Real_CreateProcessW, LPPROCESS_INFORMATION, LPSTARTUPINFOW,
            ResumeThread,
        },
        winbase::CREATE_SUSPENDED,
        winnt::{LPCWSTR, LPWSTR},
    },
};

static DETOUR_CREATE_PROCESS_W: Detour<
    unsafe extern "system" fn(
        LPCWSTR,
        LPWSTR,
        LPSECURITY_ATTRIBUTES,
        LPSECURITY_ATTRIBUTES,
        BOOL,
        DWORD,
        LPVOID,
        LPCWSTR,
        LPSTARTUPINFOW,
        LPPROCESS_INFORMATION,
    ) -> i32,
> = unsafe { Detour::new(Real_CreateProcessW, CreateProcessW) };

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

    let client = unsafe { global_client() };
    if !lpApplicationName.is_null() {
        client.send(PathAccess {
            mode: AccessMode::Read,
            path: NativeStr::from_wide(
                unsafe { U16CStr::from_ptr_str(lpApplicationName) }.as_slice(),
            ),
            dir: None,
        });
    }
    unsafe extern "system" fn CreateProcessWithPayloadW(
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
        let ret = unsafe {
            (DETOUR_CREATE_PROCESS_W.real())(
                lpApplicationName,
                lpCommandLine,
                lpProcessAttributes,
                lpThreadAttributes,
                bInheritHandles,
                dwCreationFlags | CREATE_SUSPENDED,
                lpEnvironment,
                lpCurrentDirectory,
                lpStartupInfo,
                lpProcessInformation,
            )
        };
        if ret == 0 {
            return 0;
        }

        let ret = unsafe { global_client().prepare_child_process((*lpProcessInformation).hProcess) };
        
        if ret == 0 {
            return 0;
        }
        if dwCreationFlags & CREATE_SUSPENDED == 0 {
            let ret = unsafe { ResumeThread((*lpProcessInformation).hThread) };
            if ret == -1i32 as DWORD {
                return 0;
            }
        }
        ret
    }

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
            client.asni_dll_path().as_ptr().cast(),
            Some(CreateProcessWithPayloadW),
        )
    }
}

pub const DETOURS: &[DetourAny] = &[DETOUR_CREATE_PROCESS_W.as_any()];
