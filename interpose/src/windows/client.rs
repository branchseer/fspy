use std::{
    cell::SyncUnsafeCell,
    ffi::CStr,
    fs::OpenOptions,
    mem::MaybeUninit,
    os::windows::io::{AsHandle, AsRawHandle, OwnedHandle},
    ptr::null_mut,
};

use bincode::{borrow_decode_from_slice, encode_into_std_write};
use fspy_shared::{
    ipc::{BINCODE_CONFIG, PathAccess},
    windows::Payload,
};
use smallvec::SmallVec;
use winapi::{shared::minwindef::DWORD, um::fileapi::WriteFile};
use winsafe::GetLastError;

pub struct Client<'a> {
    payload_bytes: &'a [u8],
    asni_dll_path: &'a CStr,
    ipc_pipe: OwnedHandle,
}

fn write_pipe_message(pipe: &impl AsHandle, msg: &[u8]) {
    let mut bytes_written: DWORD = 0;
    let bytes_len: DWORD = msg.len().try_into().unwrap();
    let ret = unsafe {
        WriteFile(
            pipe.as_handle().as_raw_handle().cast(),
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

impl<'a> Client<'a> {
    pub fn from_payload_bytes(payload_bytes: &'a [u8]) -> Self {
        let (payload, decoded_len) =
            borrow_decode_from_slice::<'a, Payload, _>(payload_bytes, BINCODE_CONFIG).unwrap();
        assert_eq!(decoded_len, payload_bytes.len());

        let ipc_pipe = OpenOptions::new()
            .write(true)
            .open(payload.pipe_name)
            .unwrap();

        let ipc_pipe = OwnedHandle::from(ipc_pipe);

        let asni_dll_path = CStr::from_bytes_with_nul(payload.asni_dll_path_with_nul).unwrap();
        Self {
            payload_bytes,
            asni_dll_path,
            ipc_pipe,
        }
    }
    pub fn send(&self, access: PathAccess<'_>) {
        let mut buf = SmallVec::<[u8; 256]>::new();
        encode_into_std_write(access, &mut buf, BINCODE_CONFIG).unwrap();
        write_pipe_message(&self.ipc_pipe, buf.as_slice());
    }
    pub fn payload_bytes(&self) -> &'a [u8] {
        self.payload_bytes
    }
    pub fn asni_dll_path(&self) -> &'a CStr {
        self.asni_dll_path
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
