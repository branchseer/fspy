mod raw;

pub use raw::RawCommand;

use std::{ffi::OsStr, iter::once, path::Path};

use crate::shebang::NixFileSystem;
use allocator_api2::{alloc::Allocator, vec::Vec};

use super::shebang::parse_shebang;

#[derive_where::derive_where(Debug)]
pub struct CommandInfo<'a, A: Allocator> {
    pub program: &'a Path,
    pub args: Vec<&'a OsStr, A>,
    pub envs: Vec<(&'a OsStr, &'a OsStr), A>,
}

pub struct CommandInfoRef<'a> {
    pub program: &'a Path,
    pub args: &'a [&'a OsStr],
    pub envs: &'a [(&'a OsStr, &'a OsStr)],
}

impl<'a, A: Allocator + 'a> CommandInfo<'a, A> {
    pub fn as_cmd_info_ref(&self) -> CommandInfoRef<'_> {
        CommandInfoRef {
            program: self.program,
            args: &self.args,
            envs: &self.envs,
        }
    }
    pub fn parse_shebang(&mut self, alloc: A) -> nix::Result<()> {
        // TODO: collect path accesses in fs
        if let Some(shebang) = parse_shebang(alloc, &NixFileSystem::default(), self.program)? {
            self.args[0] = shebang.interpreter.as_os_str();
            self.args.splice(
                1..1,
                shebang
                    .arguments
                    .iter()
                    .chain(once(self.program.as_os_str())),
            );
            self.program = shebang.interpreter;
        }
        Ok(())
    }
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
