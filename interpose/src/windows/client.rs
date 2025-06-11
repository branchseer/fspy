use std::{
    cell::SyncUnsafeCell, ffi::{c_void, CStr}, fs::OpenOptions, hint::black_box, mem::MaybeUninit, os::windows::io::{AsHandle, AsRawHandle, OwnedHandle, RawHandle}, ptr::{null, null_mut}
};

use bincode::{borrow_decode_from_slice, encode_into_std_write, encode_to_vec};
use fspy_shared::{
    ipc::{BINCODE_CONFIG, PathAccess},
    windows::{PAYLOAD_ID, Payload},
};
use ms_detours::DetourCopyPayloadToProcess;
use ntapi::ntobapi::DUPLICATE_SAME_ACCESS;
use smallvec::SmallVec;
use winapi::{
    shared::minwindef::{BOOL, DWORD, FALSE},
    um::{
        fileapi::WriteFile, handleapi::DuplicateHandle, processthreadsapi::GetCurrentProcess,
        winnt::HANDLE,
    },
};
use winsafe::GetLastError;

use crate::stack_once::{StackOnceGuard, stack_once_token};

pub struct Client<'a> {
    payload: Payload<'a>,
}

unsafe fn write_pipe_message(pipe: HANDLE, msg: &[u8]) {
    let mut bytes_written: DWORD = 0;
    let bytes_len: DWORD = msg.len().try_into().unwrap();
    let ret = unsafe {
        WriteFile(
            pipe,
            msg.as_ptr().cast(),
            msg.len().try_into().unwrap(),
            &mut bytes_written,
            null_mut(),
        )
    };
    assert_ne!(
        ret,
        0,
        "fspy WriteFile to pipe failed: {:?}",
        GetLastError()
    );
    assert_eq!(
        bytes_written, bytes_len,
        "fspy WriteFile to pipe not completed: {} out of {} bytes written",
        bytes_written, bytes_len
    );
}

stack_once_token!(PATH_ACCESS_ONCE);

pub struct PathAccessSender {
    pipe_handle: HANDLE,
    _once_guard: StackOnceGuard,
}

impl PathAccessSender {
    pub unsafe fn send(&self, access: PathAccess<'_>) {
       
        // TODO: send cwd as dir if the path is relative
        let mut buf = SmallVec::<u8, 256>::new();
        encode_into_std_write(access, &mut buf, BINCODE_CONFIG).unwrap();
        // black_box(buf);
        //  return;
        unsafe { write_pipe_message(self.pipe_handle, buf.as_slice()) };
    }
}

impl<'a> Client<'a> {
    pub fn from_payload_bytes(payload_bytes: &'a [u8]) -> Self {
        let (payload, decoded_len) =
            borrow_decode_from_slice::<'a, Payload, _>(payload_bytes, BINCODE_CONFIG).unwrap();
        assert_eq!(decoded_len, payload_bytes.len());

        Self { payload }
    }
    pub fn sender(&self) -> Option<PathAccessSender> {
        let guard = PATH_ACCESS_ONCE.try_enter()?;
        Some(PathAccessSender {
            pipe_handle: self.payload.pipe_handle as _,
            _once_guard: guard,
        })
    }
    pub unsafe fn prepare_child_process(&self, child_handle: HANDLE) -> BOOL {
        let mut payload = self.payload;

        let mut handle_in_child: *mut c_void = null_mut();
        let ret = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                payload.pipe_handle as _,
                child_handle,
                &mut handle_in_child,
                0,
                FALSE,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if ret == 0 {
            return 0;
        }

        payload.pipe_handle = handle_in_child as usize;

        let payload_bytes = encode_to_vec(payload, BINCODE_CONFIG).unwrap();
        unsafe {
            DetourCopyPayloadToProcess(
                child_handle,
                &PAYLOAD_ID,
                payload_bytes.as_ptr().cast(),
                payload_bytes.len().try_into().unwrap(),
            )
        }
    }
    pub fn asni_dll_path(&self) -> &'a CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(self.payload.asni_dll_path_with_nul) }
    }
}

static CLIENT: SyncUnsafeCell<MaybeUninit<Client<'static>>> =
    SyncUnsafeCell::new(MaybeUninit::uninit());

pub unsafe fn set_global_client(client: Client<'static>) {
    unsafe { *CLIENT.get() = MaybeUninit::new(client) }
}

pub unsafe fn global_client() -> &'static Client<'static> {
    unsafe { (*CLIENT.get()).assume_init_ref() }
}
