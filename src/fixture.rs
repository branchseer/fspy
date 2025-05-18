use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

pub struct Fixture {
    name: &'static str,
    content: &'static [u8],
    hash: &'static str,
}

#[doc(hidden)]
#[macro_export]
macro_rules! fixture  {
    ($name: literal) => {
        $crate::fixture::Fixture::new(
            $name,
            ::core::include_bytes!(::core::concat!(::core::env!("OUT_DIR"), "/", $name)),
            ::core::include_str!(::core::concat!(::core::env!("OUT_DIR"), "/", $name, ".hash")),
        )
    };
}

pub use fixture;

impl Fixture {
    pub const fn new(name: &'static str, content: &'static [u8], hash: &'static str) -> Self {
        Self {
            name,
            content,
            hash
        }
    }
    pub fn write_to(&self, dir: impl AsRef<Path>) -> io::Result<PathBuf> {
        const EXECUTABLE_MODE: u32 = 0o755;
        let dir = dir.as_ref();
        let path = dir.join(format!("{}_{}", self.name, self.hash));

        if fs::exists(&path)? {
            return Ok(path);
        }
        let tmp_path = dir.join(format!("{:x}", rand::random::<u128>()));
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(EXECUTABLE_MODE)
            .open(&tmp_path)?;
        tmp_file.write_all(self.content)?;
        drop(tmp_file);

        if let Err(err) = fs::rename(&tmp_path, &path) {
            if !fs::exists(&path)? {
                return Err(err);
            }
            fs::remove_file(&tmp_path)?;
        }
        Ok(path)
    }
}
