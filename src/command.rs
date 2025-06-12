use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use crate::os_impl;

pub struct Command {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) envs: HashMap<OsString, OsString>,
    pub(crate) cwd: Option<PathBuf>,
    #[cfg(unix)]
    pub(crate) arg0: Option<OsString>,

    pub(crate) spy_inner: os_impl::SpyInner,
}

impl Command {
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Command {
        self.envs.remove(key.as_ref());
        self
    }
    pub fn env_clear(&mut self) -> &mut Command {
        self.envs.clear();
        self
    }

    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Command
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.envs
            .insert(key.as_ref().to_os_string(), val.as_ref().to_os_string());
        self
    }

    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Command
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.envs.extend(
            vars.into_iter()
                .map(|(key, val)| (key.as_ref().to_os_string(), val.as_ref().to_os_string())),
        );
        self
    }
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Command {
        self.cwd = Some(dir.as_ref().to_owned());
        self
    }
    #[cfg(unix)]
    pub fn arg0<S>(&mut self, arg: S) -> &mut Command
    where
        S: AsRef<OsStr>,
    {
        self.arg0 = Some(arg.as_ref().to_os_string());
        self
    }
}

