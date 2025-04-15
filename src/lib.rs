use std::{
    env::current_dir,
    fs, io,
    path::{Path, PathBuf},
};
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

const PRELOAD_CDYLIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/preload_cdylib"));
const PRELOAD_CDYLIB_FILENAME: &str = const_format::formatc!(
    "preload_{:x}.{}",
    const_fnv1a_hash::fnv1a_hash_128(PRELOAD_CDYLIB, None),
    std::env::consts::DLL_EXTENSION,
);
fn ensure_preload_cdylib(dir: impl AsRef<std::path::Path>) -> io::Result<std::path::PathBuf> {
    let abs_dir_path = current_dir()?.join(dir.as_ref());
    let preload_dylib_path = abs_dir_path.join(PRELOAD_CDYLIB_FILENAME);
    if !fs::exists(&preload_dylib_path)? {
        let tmp_cdylib_path =
            abs_dir_path.join(format!("preload_{:x}.tmp", rand::random::<u128>()));
        fs::write(&tmp_cdylib_path, PRELOAD_CDYLIB)?;
        if let Err(err) = fs::rename(&tmp_cdylib_path, &preload_dylib_path) {
            if err.kind() != io::ErrorKind::AlreadyExists {
                let _ = fs::remove_file(&tmp_cdylib_path);
                return Err(err);
            }
        }
    }
    Ok(preload_dylib_path)
}
pub struct Tracker {
    preload_cdylib_path: PathBuf,
}
impl Tracker {
    pub fn with_fixture_dir(dir: impl AsRef<Path>) -> io::Result<Self> {
        let preload_cdylib_path = ensure_preload_cdylib(dir)?;
        Ok(Self { preload_cdylib_path })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(5, 2);
        assert_eq!(result, 4);
    }
}
