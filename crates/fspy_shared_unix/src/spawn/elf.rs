use std::{
    ffi::{CStr, OsStr},
    os::unix::ffi::OsStrExt as _,
    path::Path,
};

use elf::{ElfBytes, abi::PT_INTERP, endian::AnyEndian};

pub fn is_dynamically_linked_to_libc(executable: impl AsRef<[u8]>) -> nix::Result<bool> {
    let elf = ElfBytes::<'_, AnyEndian>::minimal_parse(executable.as_ref()).map_err(|_| nix::Error::ENOEXEC)?;
    let Some(headers) = elf.segments() else {
        return Ok(false);
    };

    let Some(interp_header) = headers
        .into_iter()
        .find(|header| header.p_type == PT_INTERP)
    else {
        return Ok(false);
    };
    let Ok(interp) = elf.segment_data(&interp_header) else {
        return Err(nix::Error::ENOEXEC);
    };

    let interp = CStr::from_bytes_until_nul(interp)
        .map(CStr::to_bytes)
        .unwrap_or(interp);

    let Some(interp_filename) = Path::new(OsStr::from_bytes(interp)).file_name() else {
        return Ok(false);
    };
    let interp_filename = interp_filename.as_bytes();
    Ok(interp_filename.starts_with(b"ld-") || interp_filename.starts_with(b"ld."))
}

#[cfg(test)]
mod tests {
    use std::fs::{read_dir, read};

    use super::*;
    #[test]
    fn dynamic_executable() {
        assert_eq!(is_dynamically_linked_to_libc(read("/bin/cat").unwrap()).unwrap(), true);
    }
    #[test]
    fn static_executable() {
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
        assert_eq!(is_dynamically_linked_to_libc(read(ld_so_path).unwrap()).unwrap(), false);
    }
}
