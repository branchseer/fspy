pub(crate) mod client;

use std::{
    borrow::Borrow as _,
    cell::{SyncUnsafeCell, UnsafeCell},
    ffi::{CStr, c_char, c_long},
    fs::OpenOptions,
    io::Write,
    mem::MaybeUninit,
    os::{raw::c_void, windows::io::AsRawHandle},
    ptr::null_mut,
    slice,
    str::from_utf8,
};

use arrayvec::ArrayVec;
use bincode::borrow_decode_from_slice;
use fspy_shared::{ipc::BINCODE_CONFIG, windows::{Payload, PAYLOAD_ID}};
use ms_detours::{
    DetourAttach, DetourCopyPayloadToProcess, DetourCreateProcessWithDllExW,
    DetourCreateProcessWithDllsW, DetourDetach, DetourFindPayloadEx, DetourIsHelperProcess,
    DetourRestoreAfterWith, DetourTransactionBegin, DetourTransactionCommit, DetourUpdateThread,
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
            LPSTARTUPINFOW, ResumeThread,
        },
        winbase::{CREATE_SUSPENDED},
        winnt::{self, LPCWSTR, LPWSTR},
    },
};
use winsafe::{GetLastError, SetLastError};

use client::{set_global_client, Client};

use crate::windows::client::global_client;

struct SyncCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

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
            (*CPW.0.get())(
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

    let dll_path = unsafe { global_client() }.asni_dll_path();
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
            dll_path.as_ptr().cast(),
            Some(CreateProcessWithPayloadW),
        )
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
            ck(unsafe { DetourRestoreAfterWith() })?;

            let mut payload_len: DWORD = 0;
            let payload_ptr =
                unsafe { DetourFindPayloadEx(&PAYLOAD_ID, &mut payload_len).cast::<u8>() };
            let payload_bytes = unsafe { slice::from_raw_parts::<'static, u8>(payload_ptr, payload_len.try_into().unwrap()) };
            let client = Client::from_payload_bytes(payload_bytes);
            unsafe { set_global_client(client) };

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
