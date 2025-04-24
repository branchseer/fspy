use std::{ffi::{c_char, CStr, CString}, iter::FusedIterator, marker::PhantomData, ptr::{null, null_mut, NonNull}, slice};

pub trait Zeroable: PartialEq {
    const ZERO: Self;
}
impl Zeroable for u8 {
    const ZERO: u8 = 0;
}
impl<T> Zeroable for *const T {
    const ZERO: *const T = null();
}
impl<T> Zeroable for *mut T {
    const ZERO: *mut T = null_mut();
}

pub struct NulTerminated<'a, T>(NonNull<T>, PhantomData<&'a [T]>);

impl<'a, T> Clone for NulTerminated<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}
impl<'a, T> Copy for NulTerminated<'a, T> { }

impl<'a, T: Zeroable> Iterator for NulTerminated<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let element = unsafe { self.0.as_ref() };
        if element.eq(&T::ZERO) {
            return None;
        }
        self.0 = unsafe { self.0.add(1) };
        Some(element)
    }
}
impl<'a, T: Zeroable> FusedIterator for NulTerminated<'a, T> {}

impl<'a, T: Zeroable> NulTerminated<'a, T> {
    pub const unsafe fn from_nullable_ptr(ptr: *const T) -> Option<Self> {
        if let Some(nonnull) = NonNull::new(ptr.cast_mut()) {
            Some(NulTerminated(nonnull, PhantomData))
        } else {
            None
        }
    }

    pub const unsafe fn from_ptr(ptr: *const T) -> Self {
        NulTerminated(unsafe { NonNull::new_unchecked(ptr.cast_mut()) }, PhantomData)
    }
    pub fn as_ptr(self) -> *const T {
        self.0.as_ptr()
    }
    pub fn to_counted(self) -> CountedNulTerminated<'a, T> {
        let len_without_term = self.count();
        CountedNulTerminated(unsafe { slice::from_raw_parts(self.0.as_ptr(), len_without_term + 1) })
    }
}

#[derive(Debug)]
pub struct CountedNulTerminated<'a, T>(&'a [T]);
impl<'a, T> Clone for CountedNulTerminated<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<'a, T> Copy for CountedNulTerminated<'a, T> { }

impl<'a, T> CountedNulTerminated<'a, T> {
    pub fn as_slice(&self) -> &'a [T] {
        &self.0[..(self.0.len() - 1)]
    }
    pub fn as_slice_with_term(&self) -> &'a [T] {
        &self.0
    }
    pub fn as_ptr(self) -> *const T {
        self.0.as_ptr()
    }
    pub fn skip(self, n: usize) -> Option<Self> {
        if n < self.0.len() {
            Some(CountedNulTerminated(&self.0[n..]))
        } else {
            None
        }
    }
}

pub type ThinCStr<'a> = NulTerminated<'a, u8>;
pub type CountedCStr<'a> = CountedNulTerminated<'a, u8>;

impl<'a> ThinCStr<'a> {
    pub fn as_c_str(self) -> &'a CStr {
        unsafe { CStr::from_ptr(self.as_ptr().cast()) }
    }
}

impl<'a> CountedCStr<'a> {
    pub fn to_c_string(self) -> CString {
        unsafe { CString::from_vec_with_nul_unchecked(self.0.to_vec()) }
    }
}

#[derive(Clone, Copy)]
pub struct Env<'a> {
    data: CountedCStr<'a>,
    eq_index: Option<usize>,
}

impl<'a> From<CountedCStr<'a>> for Env<'a> {
    fn from(data: CountedCStr<'a>) -> Self {
        let eq_index = data.as_slice().iter().position(|ch| *ch == b'=');
        Self { data, eq_index }
    }
}

impl<'a> Env<'a> {
    pub fn new_if_name_eq<S: AsRef<[u8]>>(name: S, data: ThinCStr<'a>) -> Option<Self> {
        let name = name.as_ref();
        let mut data_iter = data.copied();
        for name_char in name {
            let name_char = *name_char;
            if matches!(name_char, b'\0' | b'=') {
                return None;
            }
            if data_iter.next() != Some(name_char) {
                return None;
            };
        }
        let eq_index = match data_iter.next() {
            None => None,
            Some(b'=') => Some(name.len()),
            _ => return None
        };
        Some(Self {
            data: data.to_counted(),
            eq_index
        })
    }
    pub fn data(&self) -> CountedCStr<'a> {
        self.data
    }
    pub fn name(&self) -> &'a [u8] {
        let name_end = if let Some(eq_index) = self.eq_index {
            eq_index
        } else {
            self.data.0.len() - 1 // points to `\0`
        };
        &self.data.0[..name_end]
    }
    pub fn value(&self) -> CountedCStr<'a> {
        let value_start = if let Some(eq_index) = self.eq_index {
            eq_index + 1
        } else {
            self.data.0.len() - 1 // points to `\0`
        };
        self.data.skip(value_start).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminated_iter() {
        let terminated = unsafe { NulTerminated::from_ptr([1u8, 0].as_ptr()) };
        assert_eq!(terminated.copied().collect::<Vec<u8>>(), vec![1]);
        let terminated = unsafe { NulTerminated::from_ptr([0].as_ptr()) };
        assert_eq!(terminated.copied().collect::<Vec<u8>>(), vec![]);
    }
    #[test]
    fn terminated_to_fat() {
        let fat_terminated = unsafe { NulTerminated::from_ptr([1u8, 0].as_ptr()) }.to_counted();
        assert_eq!(fat_terminated.as_slice(), &[1]);
        assert_eq!(fat_terminated.as_slice_with_term(), &[1, 0]);

        let fat_terminated = unsafe { NulTerminated::from_ptr([0].as_ptr()) }.to_counted();
        assert_eq!(fat_terminated.as_slice(), &[]);
        assert_eq!(fat_terminated.as_slice_with_term(), &[0]);
    }
    #[test]
    fn fat_terminated_skip() {
        let fat_terminated = unsafe { NulTerminated::from_ptr([1u8, 0].as_ptr()) }.to_counted();
        assert_eq!(fat_terminated.skip(1).unwrap().as_slice_with_term(), &[0]);
        assert_eq!(fat_terminated.skip(2).is_none(), true);
    }

    const NORMAL_ENV: NulTerminated<'static, u8> = unsafe { NulTerminated::from_ptr(c"a=b".as_ptr().cast()) };
    const ENV_WITHOU_EQ: NulTerminated<'static, u8> = unsafe { NulTerminated::from_ptr(c"ab".as_ptr().cast()) };
    #[test]
    fn env_new_if_name_eq_basic() {
        let env = Env::new_if_name_eq(b"a",  NORMAL_ENV).unwrap();
        assert_eq!(env.data().as_slice_with_term(), b"a=b\0");
        assert_eq!(env.name(), b"a");
        assert_eq!(env.value().as_slice_with_term(), b"b\0");
    }
    
    #[test]
    fn env_new_if_name_without_eq() {
        let env = Env::new_if_name_eq(b"ab",  ENV_WITHOU_EQ).unwrap();
        assert_eq!(env.data().as_slice_with_term(), b"ab\0");
        assert_eq!(env.name(), b"ab");
        assert_eq!(env.value().as_slice_with_term(), b"\0");
    }
    #[test]
    fn env_new_if_name_eq_mismatch() {
        let env = Env::new_if_name_eq(b"x",  NORMAL_ENV);
        assert!(env.is_none());
        let env = Env::new_if_name_eq(b"a",  ENV_WITHOU_EQ);
        assert!(env.is_none());
    }
    #[test]
    fn env_new_if_name_eq_illegal_name() {
        let env = Env::new_if_name_eq(b"a=",  NORMAL_ENV);
        assert!(env.is_none());

        let env = Env::new_if_name_eq(b"ab\0",  ENV_WITHOU_EQ);
        assert!(env.is_none());
    }
}

pub unsafe fn iter_environ() -> impl Iterator<Item = ThinCStr<'static>> {
    unsafe extern "C" {
        static mut environ: *const *const c_char;
    }
    unsafe { iter_envp(environ) }
}

pub unsafe fn find_env<S: AsRef<[u8]>>(name: S) -> Option<Env<'static>> {
    let name = name.as_ref();
    unsafe { iter_environ() }.find_map(|data| Env::new_if_name_eq(name, data))
}

pub unsafe fn iter_envp<'a>(envp: *const *const c_char) -> impl Iterator<Item = ThinCStr<'a>> {
    unsafe { NulTerminated::<'a, *const c_char>::from_ptr(envp) }.map(|ptr| {
        unsafe { ThinCStr::from_ptr(*ptr.cast()) }
    })
}
