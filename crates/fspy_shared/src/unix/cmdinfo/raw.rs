use std::{convert::identity, ffi::{CStr, OsStr}, os::unix::ffi::OsStrExt as _, path::Path, ptr::null};

use allocator_api2::{alloc::Allocator, vec::Vec};
use super::CommandInfo;


#[derive(Clone, Copy)]
pub struct RawCommand {
    pub prog: *const libc::c_char,
    pub argv: *const *const libc::c_char,
    pub envp: *const *const libc::c_char,
}

impl RawCommand {
    unsafe fn collect_c_str_array<'a, A: Allocator + 'a, T>(
        alloc: A,
        strs: *const *const libc::c_char,
        mut map_fn: impl FnMut(&'a OsStr) -> T,
    ) -> Vec<T, A> {
        let mut count = 0usize;
        let mut cur_str = strs;
        while !(unsafe { *cur_str }).is_null() {
            count += 1;
            cur_str = unsafe { cur_str.add(1) };
        }

        let mut str_vec = Vec::<T, A>::with_capacity_in(count, alloc);
        for i in 0..count {
            let cur_str = unsafe { strs.add(i) };
            str_vec.push(map_fn(OsStr::from_bytes(
                unsafe { CStr::from_ptr(*cur_str) }.to_bytes(),
            )));
        }
        str_vec
    }
    pub fn to_c_str<A: Allocator>(alloc: A, s: &OsStr) -> *const libc::c_char {
        let s = s.as_bytes();
        let mut c_str = Vec::<u8, A>::with_capacity_in(s.len() + 1, alloc);
        c_str.extend_from_slice(s);
        c_str.push(0);
        c_str.leak().as_ptr().cast()
    }
    fn to_c_str_array<'a, A: Allocator + Copy + 'a>(
        alloc: A,
        strs: impl ExactSizeIterator<Item = &'a OsStr>,
    ) -> *const *const libc::c_char {
        let mut str_vec =
            Vec::<*const libc::c_char, A>::with_capacity_in(strs.len() + 1, alloc);
        for s in strs {
            str_vec.push(Self::to_c_str(alloc, s));
        }
        str_vec.push(null());
        str_vec.leak().as_ptr().cast()
    }

    pub unsafe fn into_command<'a, A: Allocator + Copy + 'a>(self, alloc: A) -> CommandInfo<'a, A> {
        let program = Path::new(OsStr::from_bytes(
            unsafe { CStr::from_ptr(self.prog) }.to_bytes(),
        ));

        let args = unsafe { Self::collect_c_str_array(alloc, self.argv, identity) };

        let envs = unsafe {
            Self::collect_c_str_array(alloc, self.envp, |env| {
                let env = env.as_bytes();
                let mut key: &[u8] = env;
                let mut value: &[u8] = b"";
                if let Some(eq_pos) = env.iter().position(|b| *b == b'=') {
                    key = &env[..eq_pos];
                    value = &env[(eq_pos + 1)..];
                }
                (OsStr::from_bytes(key), OsStr::from_bytes(value))
            })
        };

        CommandInfo {
            program,
            args,
            envs,
        }
    }
    pub fn from_command<'a, A: Allocator + Copy + 'a>(alloc: A, cmd: &CommandInfo<'a, A>) -> Self {
        RawCommand {
            prog: Self::to_c_str(alloc, cmd.program.as_os_str()),
            argv: Self::to_c_str_array(alloc, cmd.args.iter().copied()),
            envp: Self::to_c_str_array(
                alloc,
                cmd.envs.iter().copied().map(|(name, value)| {
                    let name = name.as_bytes();
                    let value = value.as_bytes();
                    let mut env = Vec::<u8, A>::with_capacity_in(
                        name.len() + 1 + value.len() + 1,
                        alloc,
                    );
                    env.extend_from_slice(name);
                    env.push(b'=');
                    env.extend_from_slice(value);
                    env.push(0);
                    OsStr::from_bytes(env.leak())
                }),
            ),
        }
    }
}
