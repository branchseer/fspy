use std::{ffi::OsStr, path::Path};

use allocator_api2::{alloc::Allocator, vec::Vec};

#[derive_where::derive_where(Debug)]
pub struct CommandInfo<'a, A: Allocator> {
    pub program: &'a Path,
    pub args: Vec<&'a OsStr, A>,
    pub envs: Vec<(&'a OsStr, &'a OsStr), A>,
}

pub fn ensure_env<'a, A: Allocator + 'a>(
    envs: &mut Vec<(&'a OsStr, &'a OsStr), A>,
    name: &'a OsStr,
    value: &'a OsStr,
) -> nix::Result<()> {
    let existing_value = envs
        .iter()
        .copied()
        .find_map(|(n, v)| if n == name { Some(v) } else { None });
    if let Some(existing_value) = existing_value {
        return if existing_value == value {
            Ok(())
        } else {
            Err(nix::Error::EINVAL)
        };
    };
    envs.push((name, value));
    Ok(())
}
