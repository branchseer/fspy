use std::path::PathBuf;

mod linux;
mod command_builder;

pub struct FileSystemAccess {
    path: PathBuf
}
