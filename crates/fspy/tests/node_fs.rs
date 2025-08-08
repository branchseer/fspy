use std::{env::current_dir, io};

use fspy::{AccessMode, PathAccess, PathAccessIterable, TrackedChild};

async fn track_node_script(script: &str) -> io::Result<PathAccessIterable> {
    let mut command = fspy::Spy::global()?.new_command("/home/vscode/node");
    command.arg("-e").arg(script);
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
async fn read_sync() -> io::Result<()> {
    let accesses = track_node_script("try { fs.readFileSync('hello') } catch {}").await?;
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
async fn read_dir_sync() -> io::Result<()> {
    let accesses = track_node_script("try { fs.readdirSync('hello') } catch {}").await?;
    assert_contains(
        &accesses,
        &PathAccess {
            mode: AccessMode::ReadDir,
            path: current_dir().unwrap().join("hello").as_path().into(),
        },
    );
    Ok(())
}
