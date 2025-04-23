use std::{fs::File, io::Write, mem::ManuallyDrop, os::fd::FromRawFd};

pub fn abort_with(msgs: &[impl AsRef<[u8]>]) -> ! {
    let mut stderr = ManuallyDrop::new(unsafe { File::from_raw_fd(libc::STDERR_FILENO) });
    for m in msgs {
        let _ = stderr.write_all(m.as_ref());
    }
    let _ = stderr.write_all(b"\n");
    unsafe { libc::abort() }
}
