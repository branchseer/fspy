use std::{
    cell::SyncUnsafeCell,
    ffi::CStr,
    fs::{File, OpenOptions},
    io::Write as _,
    mem::MaybeUninit,
};

use bincode::borrow_decode_from_slice;
use fspy_shared::{ipc::BINCODE_CONFIG, windows::Payload};

pub struct Client<'a> {
    payload_bytes: &'a [u8],
    asni_dll_path: &'a CStr,
    ipc_pipe: File,
}

impl<'a> Client<'a> {
    pub fn from_payload_bytes(payload_bytes: &'a [u8]) -> Self {
        let (payload, decoded_len) =
            borrow_decode_from_slice::<'a, Payload, _>(payload_bytes, BINCODE_CONFIG).unwrap();
        assert_eq!(decoded_len, payload_bytes.len());

        let mut ipc_pipe = OpenOptions::new()
            .write(true)
            .open(payload.pipe_name)
            .unwrap();

        ipc_pipe.write(b"hello").unwrap();

        let asni_dll_path = CStr::from_bytes_with_nul(payload.asni_dll_path_with_nul).unwrap();
        Self {
            payload_bytes,
            asni_dll_path,
            ipc_pipe,
        }
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
