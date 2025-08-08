mod sys;
mod which;
mod fs;
mod shebang;

use bstr::{BStr, BString};
use ::which::{
    WhichConfig,
    sys::{RealSys, Sys},
};

use std::{
    ffi::{OsStr, OsString},
    io,
    iter::once,
    mem::replace,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

use shebang::{NixFileSystem, ParseShebangOptions, ShebangParseFileSystem};

use shebang::parse_shebang;

pub use sys::real_sys_with_callback;

#[derive(Debug, Clone)]
pub struct SearchPath {
    /// Custom search path to use (like execvP), overrides PATH if Some
    pub custom_path: Option<PathBuf>,
}

/// Configuration for exec resolution behavior
#[derive(Debug, Clone)]
pub struct ExecResolveConfig {
    /// If Some and the program doesn't contains `/`,
    /// search the program in PATH (like execvp, execvpe, execlp) instead of finding it in current directory
    pub search_path: Option<SearchPath>,
    /// Options for parsing shebangs (all exec variants handle shebangs)
    pub shebang_options: ParseShebangOptions,
}

impl ExecResolveConfig {
    /// Configuration for execve - no PATH search, direct execution
    pub fn search_path_disabled() -> Self {
        Self {
            search_path: None,
            shebang_options: Default::default(),
        }
    }
    /// execlp/execvp/execvP/execvpe
    /// `custom_path` allows a customized path to be searched like in execvP (macOS extension)
    pub fn search_path_enabled(custom_path: Option<PathBuf>) -> Self {
        Self {
            search_path: Some(SearchPath { custom_path }),
            shebang_options: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct Exec {
    pub program: BString,
    pub args: Vec<BString>,
    /// vec of (name, value). value is None when the entry in environ doesn't contain a `=` character.
    pub envs: Vec<(BString, Option<BString>)>,
}

fn which_error_to_errno(_which_error: ::which::Error) -> nix::Error {
    nix::Error::ENOENT
}

impl Exec {
    /// Resolve the program path according to exec family semantics
    ///
    /// This method replicates the behavior of execve/execvp/execvP/execvpe for program resolution,
    /// including PATH searching and shebang handling.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if resolution succeeds and `self` is updated with resolved paths
    /// * `Err(nix::Error)` with appropriate errno, like the exec function would return
    pub fn resolve(
        &mut self,
        sys: &(impl Sys + ShebangParseFileSystem<Error = nix::Error>),
        config: ExecResolveConfig,
    ) -> nix::Result<()>
    {
        if let Some(search_path) = config.search_path {
            let mut which_config = WhichConfig::new_with_sys(sys)
                .binary_name(OsString::from_vec(self.program.clone().into()));
            if let Some(custom_path) = search_path.custom_path {
                which_config = which_config.custom_path_list(custom_path.into_os_string());
            }
            let program = which_config
                .binary_name(OsString::from_vec(self.program.clone().into()))
                .first_result()
                .map_err(which_error_to_errno)?;
            self.program = program.into_os_string().into_vec().into();
        }

        self.parse_shebang(sys, config.shebang_options)?;

        Ok(())
    }

    fn parse_shebang(
        &mut self,
        fs: &impl ShebangParseFileSystem<Error = nix::Error>,
        options: ParseShebangOptions,
    ) -> nix::Result<()> {
        if let Some(shebang) =
            parse_shebang(fs, Path::new(OsStr::from_bytes(&self.program)), options)?
        {
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
