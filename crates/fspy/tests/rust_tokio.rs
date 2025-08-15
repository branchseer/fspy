use std::{env::current_dir, io, path::Path};

use dunce::simplified;
use fspy::{AccessMode, PathAccess, PathAccessIterable, TrackedChild};
use tokio::fs::OpenOptions;

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
fn assert_contains(accesses: &PathAccessIterable, expected_path: &Path, expected_mode: AccessMode) {
    accesses
        .iter()
        .find(|access| {
            simplified(Path::new(&access.path.to_cow_os_str())) == simplified(expected_path)
                && access.mode == expected_mode
        })
        .unwrap();
}


#[tokio::test]
async fn open_read() -> io::Result<()> {
    let accesses = track_child!({
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap()
            .block_on(async {
                tokio::fs::File::open("hello").await;
            });
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

        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap()
            .block_on(async {
                OpenOptions::new().write(true).open(path).await;
            });
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

        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap()
            .block_on(async {
                tokio::fs::read_dir(path).await;
            });
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
