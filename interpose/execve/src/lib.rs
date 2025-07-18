use std::{
    convert::Infallible, ffi::CStr, num::NonZeroUsize, os::raw::c_char, path::Path, ptr::null,
};

use nix::{
    fcntl::OFlag,
    sys::{
        mman::{MapFlags, ProtFlags, mmap},
        stat::{Mode, fstat},
    },
};

unsafe extern "C" {
    unsafe fn reflect_execve(elf: *const u8, argv: *const *const c_char, env: *const *const c_char);
    static environ: *const *const c_char;
}

pub unsafe fn execve(
    path: impl AsRef<Path>,
    argv: impl Iterator<Item = impl AsRef<CStr>>,
    env: impl Iterator<Item = impl AsRef<CStr>>,
) -> nix::Result<Infallible> {
    let fd = nix::fcntl::open(path.as_ref(), OFlag::O_RDONLY, Mode::empty())?;
    let stat = fstat(&fd)?;
    let data = unsafe {
        mmap(
            None,
            NonZeroUsize::new(usize::try_from(stat.st_size).unwrap()).unwrap(),
            ProtFlags::PROT_READ,
            MapFlags::MAP_PRIVATE,
            fd,
            0,
        )
    }?;
    let argv = argv.collect::<Vec<_>>();
    let mut argv = argv
        .iter()
        .map(|arg| arg.as_ref().as_ptr())
        .collect::<Vec<_>>();
    argv.push(null());

    let env = env.collect::<Vec<_>>();
    let mut env = env
        .iter()
        .map(|env| env.as_ref().as_ptr())
        .collect::<Vec<_>>();
    env.push(null());

    unsafe {
        reflect_execve(data.as_ptr().cast(), argv.as_ptr(), env.as_mut_ptr());
    }
    unreachable!()
}
