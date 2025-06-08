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
use fspy_shared::windows::FSSPY_IPC_PAYLOAD;
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

struct SyncCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

static DLL_PATH: SyncCell<[u8; MAX_PATH]> = SyncCell(UnsafeCell::new([0; MAX_PATH]));

static FSPY_IPC: SyncUnsafeCell<MaybeUninit<&'static [u8]>> =
    SyncUnsafeCell::new(MaybeUninit::uninit());

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
        let ret = unsafe {
            let ipc_payload = (*FSPY_IPC.get()).assume_init();
            DetourCopyPayloadToProcess(
                (*lpProcessInformation).hProcess,
                &FSSPY_IPC_PAYLOAD,
                ipc_payload.as_ptr().cast(),
                ipc_payload.len().try_into().unwrap(),
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
            (*DLL_PATH.0.get()).as_ptr().cast(),
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

            let mut size: DWORD = 0;
            let ipc_ptr =
                unsafe { DetourFindPayloadEx(&FSSPY_IPC_PAYLOAD, &mut size).cast::<u8>() };
            let ipc = unsafe { slice::from_raw_parts(ipc_ptr, size.try_into().unwrap()) };
            unsafe { *FSPY_IPC.get() = MaybeUninit::new(ipc) };

            let mut ipc_pipe = OpenOptions::new()
                .write(true)
                .open(from_utf8(ipc).unwrap())
                .unwrap();

            ipc_pipe.write(b"hello").unwrap();

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
