use std::{collections::HashSet, env::args, ffi::OsString, io};

use fspy::Spy;
use futures_util::{future::join, TryStreamExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let spy = Spy::init()?;

    let mut args = args();
    args.next().unwrap(); // argv0
    let program = args.next().unwrap();
    let (mut command, mut stream) = spy.new_command(program, |command| {
        command.args(args);
        Ok(())
    })?;

    let status_fut = async move {
        let status = command.status().await;
        drop(command);
        status
    };

    let stream_fut = async move {
        let mut paths = HashSet::<OsString>::new();
        while let Some(access) = stream.try_next().await? {
            paths.insert(access.path.into_os_string());
        }
        io::Result::Ok(paths.len())
    };

    let (status, path_count) = join(status_fut, stream_fut).await;
    let status = status?;
    let path_count = path_count?;
    println!("exit with: {:?}. path count: {}", status.code(), path_count);
    Ok(())
}
