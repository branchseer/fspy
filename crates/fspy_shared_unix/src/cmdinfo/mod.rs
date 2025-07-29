mod raw;

use bstr::{BStr, BString};

use std::{ffi::OsStr, iter::once, mem::replace, os::unix::ffi::OsStrExt, path::Path};

use crate::shebang::NixFileSystem;

use super::shebang::parse_shebang;

#[derive(Debug)]
pub struct CommandInfo {
    pub program: BString,
    pub args: Vec<BString>,
    /// vec of (name, value). value is None when the entry in environ doesn't contain a `=` character.
    pub envs: Vec<(BString, Option<BString>)>,
}

impl CommandInfo {
    pub fn parse_shebang(&mut self) -> nix::Result<()> {
        // TODO: collect path accesses in fs
        if let Some(shebang) = parse_shebang(
            &NixFileSystem::default(),
            Path::new(OsStr::from_bytes(&self.program)),
            Default::default(),
        )? {
            self.args[0] = shebang.interpreter.clone();
            let old_program = replace(&mut self.program, shebang.interpreter);
            self.args
                .splice(1..1, shebang.arguments.into_iter().chain(once(old_program)));
        }
        Ok(())
    }
}

pub fn ensure_env(
    envs: &mut Vec<(BString, Option<BString>)>,
    name: impl AsRef<BStr>,
    value: impl AsRef<BStr>,
) -> nix::Result<()> {
    let name = name.as_ref();
    let value = value.as_ref();
    let existing_value = envs
        .iter()
        .find_map(|(n, v)| if n == name { v.as_ref() } else { None });
    if let Some(existing_value) = existing_value {
        return if existing_value == value {
            Ok(())
        } else {
            Err(nix::Error::EINVAL)
        };
    };
    envs.push((name.to_owned(), Some(value.to_owned())));
    Ok(())
}
