use crate::windows::{client::global_client, detour::{Detour, DetourAny}};

use fspy_shared::{ipc::{AccessMode, NativeStr, PathAccess}, windows::PAYLOAD_ID};
use ms_detours::{DetourCopyPayloadToProcess, DetourCreateProcessWithDllExW};
use widestring::U16CStr;
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
            LPSTARTUPINFOW, ResumeThread,
        },
        winbase::CREATE_SUSPENDED,
        winnt::{self, LPCWSTR, LPWSTR},
    },
};
use winsafe::{GetLastError, SetLastError, SysResult};

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
    client.send(PathAccess {
        mode: AccessMode::Read,
        path: NativeStr::from_wide(unsafe { U16CStr::from_ptr_str(lpApplicationName) }.as_slice()),
    });
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
        let payload_bytes = unsafe { global_client() }.payload_bytes();
        let ret = unsafe {
            DetourCopyPayloadToProcess(
                (*lpProcessInformation).hProcess,
                &PAYLOAD_ID,
                payload_bytes.as_ptr().cast(),
                payload_bytes.len().try_into().unwrap(),
            )
        };
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
