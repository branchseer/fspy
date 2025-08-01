mod real;
pub trait HandleSpawnFileSystem: crate::shebang::FileSystem<Error = nix::Error> + which::sys::Sys {}

pub use real::RealHandleSpawnFileSystem;

