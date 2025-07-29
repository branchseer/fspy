use std::{fs::File, io, ops::Deref, os::unix::ffi::OsStrExt as _, path::Path};

use goblin::elf::Elf;

pub fn is_dynamically_linked_to_libc(executable_path: impl AsRef<Path>) -> anyhow::Result<bool> {
    let file = File::open(executable_path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }?;
    let elf = Elf::parse(mmap.deref())?;
    let Some(interpreter) = elf.interpreter else {
        return Ok(false);
    };
    let Some(interpreter_filename) = Path::new(interpreter).file_name() else {
        return Ok(false);
    };
    let interpreter_filename = interpreter_filename.as_bytes();
    Ok(interpreter_filename.starts_with(b"ld-") || interpreter_filename.starts_with(b"ld."))
}

#[cfg(test)]
mod tests {
    use std::fs::read_dir;

    use super::*;
    #[test]
    fn is_dynamically_linked_to_libc_true() {
        assert_eq!(is_dynamically_linked_to_libc("/bin/cat").unwrap(), true);
    }
    #[test]
    fn is_dynamically_linked_to_libc_false() {
        let ld_so_filename = read_dir("/lib")
            .unwrap()
            .find_map(|entry| {
                let filename = entry.unwrap().file_name();
                let filename = filename.to_str().unwrap();
                if filename.starts_with("ld-") {
                    Some(filename.to_owned())
                } else {
                    None
                }
            })
            .unwrap();
        let ld_so_path = format!("/lib/{}", ld_so_filename);
        assert_eq!(is_dynamically_linked_to_libc(ld_so_path).unwrap(), false);
    }
}
