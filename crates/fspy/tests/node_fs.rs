use std::{
    env::{current_dir, vars_os},
    io, path::Path,
};

use dunce::simplified;
use fspy::{AccessMode, PathAccess, PathAccessIterable, TrackedChild};

async fn track_node_script(script: &str) -> io::Result<PathAccessIterable> {
    let mut command = fspy::Spy::global()?.new_command("node");
    command
        .arg("-e")
        .envs(vars_os()) // https://github.com/jdx/mise/discussions/5968
        .arg(script);
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
            dbg!(&access);
            simplified(Path::new(&access.path.to_cow_os_str())) == simplified(expected_path)
                && access.mode == expected_mode
        })
        .unwrap();
}

#[tokio::test]
async fn read_sync() -> io::Result<()> {
    let accesses = track_node_script("try { fs.readFileSync('hello') } catch {}").await?;
    assert_contains(
        &accesses,
        current_dir().unwrap().join("hello").as_path(),
        AccessMode::Read,
    );
    Ok(())
}
#[tokio::test]
async fn read_dir_sync() -> io::Result<()> {
    let accesses = track_node_script("try { fs.readdirSync('hello') } catch {}").await?;
    assert_contains(
        &accesses,
        current_dir().unwrap().join("hello").as_path(),
            AccessMode::ReadDir,
    );
    Ok(())
}
