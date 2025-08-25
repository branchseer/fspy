mod test_utils;

use test_utils::{assert_contains, track_child};
use std::{
    env::current_dir,
    ffi::OsStr,
    fs::{File, OpenOptions},
    io,
    path::Path,
};

use fspy::{AccessMode, PathAccess, PathAccessIterable, TrackedChild};

#[tokio::test]
async fn open_read() -> io::Result<()> {
    let accesses = track_child!({
        File::open("hello");
    })
    .await?;
    assert_contains(
        &accesses,
        current_dir().unwrap().join("hello").as_path(),
        AccessMode::Read,
    );

    Ok(())
}

#[tokio::test]
async fn open_write() -> io::Result<()> {
    let accesses = track_child!({
        let path = format!("{}/hello", env!("CARGO_TARGET_TMPDIR"));
        OpenOptions::new().write(true).open(path);
    })
    .await?;
    assert_contains(
        &accesses,
        Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("hello")
            .as_path(),
        AccessMode::Write,
    );

    Ok(())
}

#[tokio::test]
async fn readdir() -> io::Result<()> {
    let accesses = track_child!({
        let path = format!("{}/hello", env!("CARGO_TARGET_TMPDIR"));
        std::fs::read_dir(path);
    })
    .await?;
    assert_contains(
        &accesses,
        Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("hello")
            .as_path(),
        AccessMode::ReadDir,
    );

    Ok(())
}
