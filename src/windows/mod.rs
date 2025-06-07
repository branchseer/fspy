use std::{
    env::temp_dir,
    ffi::c_char,
    fs::create_dir,
    io,
    os::windows::{ffi::OsStrExt, io::AsRawHandle, process::ChildExt as _},
    str::from_utf8,
};

use fspy_shared::windows::FSSPY_IPC_PAYLOAD;
use ms_detours::{DetourCopyPayloadToProcess, DetourUpdateProcessWithDll};
use tokio::process::{Child, Command};
// use detours_sys2::{DetourAttach,};

use winapi::{
    shared::minwindef::TRUE,
    um::{processthreadsapi::ResumeThread, winbase::CREATE_SUSPENDED},
};
// use windows_sys::Win32::System::Threading::{CREATE_SUSPENDED, ResumeThread};
use winsafe::co::{CP, WC};

use crate::fixture::{Fixture, fixture};

const INTERPOSE_CDYLIB: Fixture = fixture!("fspy_interpose");

pub fn spawn(mut command: Command) -> io::Result<Child> {
    let tmp_dir = temp_dir().join("fspy");
    let _ = create_dir(&tmp_dir);
    let interpose_cdylib = INTERPOSE_CDYLIB.write_to(&tmp_dir, ".dll").unwrap();

    let interpose_cdylib = interpose_cdylib
        .as_os_str()
        .encode_wide()
        .collect::<Vec<u16>>();
    let mut interpose_cdylib =
        winsafe::WideCharToMultiByte(CP::ACP, WC::NoValue, &interpose_cdylib, None, None)
            .map_err(|err| io::Error::from_raw_os_error(err.raw() as i32))?;

    interpose_cdylib.push(0);

    command.creation_flags(CREATE_SUSPENDED);

    command.spawn_with(|std_command| {
        let std_child = std_command.spawn()?;

        let mut interpose_cdylib = interpose_cdylib.as_ptr().cast::<c_char>();
        let process_handle = std_child.as_raw_handle().cast::<winapi::ctypes::c_void>();
        let success =
            unsafe { DetourUpdateProcessWithDll(process_handle, &mut interpose_cdylib, 1) };
        if success != TRUE {
            return Err(io::Error::last_os_error());
        }

        let ipc_name = b"hello_ipc";

        let success = unsafe {
            DetourCopyPayloadToProcess(
                process_handle,
                &FSSPY_IPC_PAYLOAD,
                ipc_name.as_ptr().cast(),
                ipc_name.len().try_into().unwrap(),
            )
        };
        if success != TRUE {
            return Err(io::Error::last_os_error());
        }

        let main_thread_handle = std_child.main_thread_handle();
        let resume_thread_ret =
            unsafe { ResumeThread(main_thread_handle.as_raw_handle().cast()) } as i32;

        if resume_thread_ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(std_child)
    })
}
