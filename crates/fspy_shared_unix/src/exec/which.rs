use std::mem::{MaybeUninit, transmute};

use bstr::{BStr, ByteSlice};
use stackalloc::alloca;

fn concat<R>(s: &[&BStr], callback: impl FnOnce(&BStr) -> R) -> R {
    let size = s.iter().map(|s| s.len()).sum();
    alloca(size, |buf| {
        debug_assert_eq!(buf.len(), size);
        let mut pos = 0usize;
        for s in s {
            let next_pos = pos + s.len();
            buf[pos..next_pos]
                .copy_from_slice(unsafe { transmute::<&[u8], &[MaybeUninit<u8>]>(s.as_ref()) });
            pos = next_pos;
        }
        debug_assert_eq!(pos, buf.len());
        callback(unsafe { transmute::<&[MaybeUninit<u8>], &[u8]>(buf) }.as_bstr())
    })
}

// https://github.com/kraj/musl/blob/1b06420abdf46f7d06ab4067e7c51b8b63731852/src/process/execvp.c#L5
const NAME_MAX: usize = 255;
pub fn which(
    file: &BStr,
    path: &BStr,
    mut access_executable: impl FnMut(&BStr) -> nix::Result<()>,
    callback: impl FnOnce(&BStr) -> nix::Result<()>,
) -> nix::Result<()> {
    if file.contains(&b'/') {
        return callback(file);
    };
    if file.len() > NAME_MAX {
        return Err(nix::Error::ENAMETOOLONG);
    };

    let mut seen_eacces = false;
    let mut last_err = nix::Error::ENOENT;
    let mut callback = Some(callback);
    for p in path.split(|ch| *ch == b':') {
        let p = p.as_bstr();
        let result_to_return = concat(&[p, "/".into(), file], |path| {
            match access_executable(path) {
                Ok(()) => Some((callback.take().unwrap())(path)),
                Err(err @ (nix::Error::EACCES | nix::Error::ENONET | nix::Error::ENOTDIR)) => {
                    seen_eacces |= err == nix::Error::EACCES;
                    last_err = err;
                    None
                }
                Err(other_err) => Some(Err(other_err)),
            }
        });
        if let Some(result) = result_to_return {
            return result;
        }
    }
    Err(if seen_eacces {
        nix::Error::EACCES
    } else {
        last_err
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concat() {
        let s = concat(&["a".into(), "bc".into(), "".into(), "e".into()], |s| {
            s.to_owned()
        });
        assert_eq!(s, "abce");
    }
}
