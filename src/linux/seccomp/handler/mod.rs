use std::io;

use libc::seccomp_notif;
use seccompiler::SeccompFilter;

pub mod arg;

pub trait SeccompNotifyHandler {
    fn syscalls() -> &'static [syscalls::Sysno];
    fn handle_notify(&self, notify: &seccomp_notif) -> io::Result<()>;
}

macro_rules! impl_handler {
    ($type: ty, $($method:ident)*) => {

    impl $crate::os_impl::seccomp::handler::SeccompNotifyHandler for $type {
        fn syscalls() -> &'static [::syscalls::Sysno] {
            &[ $( ::syscalls::Sysno:: $method ),* ]
        }
        fn handle_notify(&self, notify: &::libc::seccomp_notif) -> ::std::io::Result<()> {
            $(
                if notify.data.nr == ::syscalls::Sysno::$method as _ {
                    return self.$method($crate::os_impl::seccomp::handler::arg::FromNotify::from_notify(notify)?)
                }
            )*
            Ok(())
        }
    }
    };
}

pub(crate) use impl_handler;
