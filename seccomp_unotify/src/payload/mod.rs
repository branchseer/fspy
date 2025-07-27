use std::os::fd::OwnedFd;
mod filter;
pub use filter::Filter;

pub struct Payload {
    pub ipc_fd: OwnedFd,
    pub filter: Filter,
}
