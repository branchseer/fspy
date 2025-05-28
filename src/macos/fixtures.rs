use phf::phf_set;

use crate::fixture::{fixture, Fixture};

pub const COREUTILS_BINARY: Fixture = fixture!("coreutils");
pub const BRUSH_BINARY: Fixture = fixture!("brush");
pub const INTERPOSE_CDYLIB: Fixture = fixture!("fspy_interpose");

#[cfg(test)]
mod tests {
    use std::{process::Command, str::from_utf8};

    use super::*;
    use super::super::command::COREUTILS_FUNCTIONS;

    #[test]
    fn coreutils_functions() {
        let tmpdir = tempfile::tempdir().unwrap();
        let coreutils_path = COREUTILS_BINARY.write_to(&tmpdir).unwrap();
        let output = Command::new(coreutils_path).arg("--list").output().unwrap();
        let mut expected_functions: Vec<&str> = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| {
                let line = line.trim_ascii();
                if line.is_empty() {
                    None
                } else {
                    Some(from_utf8(line).unwrap())
                }
            })
            .collect();
        let mut actual_functions: Vec<&str> = COREUTILS_FUNCTIONS.iter().copied().map(|f| from_utf8(f).unwrap()).collect();

        expected_functions.sort_unstable();
        actual_functions.sort_unstable();
        assert_eq!(expected_functions, actual_functions);
    }
}
