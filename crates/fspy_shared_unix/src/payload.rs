use base64::{Engine as _, prelude::BASE64_STANDARD_NO_PAD};
use bincode::{Decode, Encode, config::standard};
use std::{
    ffi::OsString,
    os::{
        fd::RawFd,
        unix::ffi::{OsStrExt as _, OsStringExt as _},
    },
};

#[derive(Debug, Encode, Decode)]
pub struct Payload {
    pub ipc_fd: RawFd,
    pub preload_path: String,
}

pub(crate) const PAYLOAD_ENV_NAME: &str = "FSPY_PAYLOAD";

pub struct EncodedPayload {
    pub payload: Payload,
    pub encoded_string: OsString,
}

pub fn encode_payload(payload: Payload) -> EncodedPayload {
    let bincode_bytes = bincode::encode_to_vec(&payload, standard()).unwrap();
    let encoded_string = BASE64_STANDARD_NO_PAD.encode(&bincode_bytes);
    EncodedPayload {
        payload,
        encoded_string: OsString::from_vec(encoded_string.into()),
    }
}

pub fn decode_payload_from_env() -> anyhow::Result<EncodedPayload> {
    let Some(encoded_string) = std::env::var_os(PAYLOAD_ENV_NAME) else {
        anyhow::bail!("Environemnt variable '{}' not found", PAYLOAD_ENV_NAME);
    };
    decode_payload(encoded_string)
}

fn decode_payload(encoded_string: OsString) -> anyhow::Result<EncodedPayload> {
    let bincode_bytes = BASE64_STANDARD_NO_PAD.decode(encoded_string.as_bytes())?;
    let (payload, n) = bincode::decode_from_slice::<Payload, _>(&bincode_bytes, standard())?;
    assert_eq!(bincode_bytes.len(), n);
    Ok(EncodedPayload {
        payload,
        encoded_string,
    })
}
