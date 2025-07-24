pub mod arg;

use std::io;
use arg::FromSyscallArg;
use libc::seccomp_notif;
use seccompiler::SeccompFilter;

pub trait SeccompNotifyHandler {
    fn syscalls() -> &'static [syscalls::Sysno];
    fn handle_notify(&mut self, notify: &seccomp_notif) -> io::Result<()>;
}

#[macro_export]
macro_rules! impl_handler {
    ($type: ty, $($method:ident)*) => {

    impl $crate::handler::SeccompNotifyHandler for $type {
        fn syscalls() -> &'static [::syscalls::Sysno] {
            &[ $( ::syscalls::Sysno:: $method ),* ]
        }
        fn handle_notify(&mut self, notify: &::libc::seccomp_notif) -> ::std::io::Result<()> {
            $(
                if notify.data.nr == ::syscalls::Sysno::$method as _ {
                    return self.$method($crate::handler::arg::FromNotify::from_notify(notify)?)
                }
            )*
            Ok(())
        }
    }
    };
}

