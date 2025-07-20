#[macro_export]
macro_rules! interpose {
    ($name: ident (64): $fn_sig: ty) => {
        $crate::interpose_inner! {
            $name: $fn_sig;

            #[cfg(test)]
            #[test]
            fn symbol_64_exists() {
               ::core::assert!(!unsafe { ::libc::dlsym(
                    ::libc::RTLD_NEXT,
                    ::core::concat!(::core::stringify!($name), "64\0").as_ptr(),
                ) }.is_null())
            }
        }
        const _: () = {
            #[unsafe(naked)]
            #[unsafe(export_name = ::core::concat!(::core::stringify!($name), 64))]
            unsafe extern "C" fn interpose_fn() {
        ::core::arch::naked_asm!("b {}", sym $name);
            }
        };
    };
    ($name: ident: $fn_sig: ty) => {

        $crate::interpose_inner! {
            $name: $fn_sig;

            #[cfg(test)]
            #[test]
            fn symbol_64_does_not_exist() {
               ::core::assert!(unsafe { ::libc::dlsym(
                    ::libc::RTLD_NEXT,
                    ::core::concat!(::core::stringify!($name), "64\0").as_ptr(),
                ) }.is_null())
            }
        }
    }
}

#[doc(hidden)]
pub fn symbol_exists() {}

#[doc(hidden)]
#[macro_export]
macro_rules! interpose_inner {
    ($name: ident: $fn_sig: ty; $test_fn: item ) => {
        const _: $fn_sig = $name;
        const _: $fn_sig = $crate::unix::libc::$name;

        #[cfg(not(test))] // Don't interpose on the test binary
        const _: () = {
            #[unsafe(naked)]
            #[unsafe(export_name = ::core::stringify!($name))]
            unsafe extern "C" fn interpose_fn() {
        ::core::arch::naked_asm!("b {}", sym $name);
            }
        };
        mod $name {
            use super::*;
            pub fn original() -> $fn_sig {
                static LAZY: std::sync::LazyLock<$fn_sig> = std::sync::LazyLock::new(|| unsafe {
                    ::core::mem::transmute_copy(&libc::dlsym(
                        libc::RTLD_NEXT,
                        ::core::concat!(::core::stringify!($name), "\0").as_ptr(),
                    ))
                });
                *LAZY
            }
            $test_fn
        }
    };
}
