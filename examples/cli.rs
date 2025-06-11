use std::{
    env::{self, args_os},
    ffi::OsStr,
    io,
    pin::{Pin, pin},
    process::ExitCode,
};

use fspy::{AccessMode, TracedProcess};
use futures_util::future::{Either, select};
use tokio::{
    fs::File,
    io::{AsyncWrite, stdout},
    process::Command,
};
use which::which;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args = args_os();
    let _ = args.next();
    assert_eq!(args.next().as_deref(), Some(OsStr::new("-o")));

    let out_path = args.next().unwrap();

    let program = args.next().unwrap();
    let program = which(program).unwrap();
    let mut command = Command::new(program);
    command.args(args);

    let TracedProcess {
        mut child,
        mut path_access_stream,
    } = fspy::spawn(command).await?;

    let out_file: Pin<Box<dyn AsyncWrite>> = if out_path == "-" {
        Box::pin(stdout())
    } else {
        Box::pin(File::create(out_path).await?)
    };

    let mut csv_writer = csv_async::AsyncWriter::from_writer(out_file);

    let mut buf = Vec::new();
    let mut path_count = 0usize;
    while let Some(acc) = path_access_stream.next(&mut buf).await? {
        path_count += 1;
        csv_writer
            .write_record(&[
                acc.path.to_cow_os_str().to_string_lossy().as_ref().as_bytes(),
                match acc.mode {
                    AccessMode::Read => b"r",
                    AccessMode::ReadWrite => b"rw",
                    AccessMode::Write => b"w",
                },
            ])
            .await?;
    }
    csv_writer.flush().await?;

    let output = child.wait().await?;
    eprintln!("\nfspy: {} paths accessed. {}", path_count, output);
    Ok(())
}
