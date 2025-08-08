use std::{
    env::current_dir,
    fs::{File, OpenOptions},
    io,
    path::Path,
};

use fspy::{AccessMode, PathAccess, PathAccessIterable, TrackedChild};

macro_rules! track_child {
    ($body: block) => {{
        const ID: &str = ::core::concat!(
            ::core::file!(),
            ":",
            ::core::line!(),
            ":",
            ::core::column!()
        );
        #[ctor::ctor]
        unsafe fn init() {
            let mut args = ::std::env::args();
            let Some(_) = args.next() else {
                return;
            };
            let Some(current_id) = args.next() else {
                return;
            };
            if current_id == ID {
                $body;
                ::std::process::exit(0);
            }
        }
        spawn_with_id(ID)
    }};
}

async fn spawn_with_id(id: &str) -> io::Result<PathAccessIterable> {
    let mut command = fspy::Spy::global()?.new_command(::std::env::current_exe()?);
    command.arg(id);
    let TrackedChild {
        mut tokio_child,
        accesses_future,
    } = command.spawn().await?;

    let acceses = accesses_future.await?;
    let status = tokio_child.wait().await?;
    assert!(status.success());
    Ok(acceses)
}

#[track_caller]
fn assert_contains(accesses: &PathAccessIterable, expected: &PathAccess<'_>) {
    accesses.iter().find(|access| access == expected).unwrap();
}

#[tokio::test]
async fn open_read() -> io::Result<()> {
    let accesses = track_child!({
        File::open("hello");
    })
    .await?;
    assert_contains(
        &accesses,
        &PathAccess {
            mode: AccessMode::Read,
            path: current_dir().unwrap().join("hello").as_path().into(),
        },
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
        &PathAccess {
            mode: AccessMode::Write,
            path: Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("hello")
                .as_path()
                .into(),
        },
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
        &PathAccess {
            mode: AccessMode::ReadDir,
            path: Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("hello")
                .as_path()
                .into(),
        },
    );

    Ok(())
}
