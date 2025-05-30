use std::{
    env::{self, args_os},
    ffi::OsStr,
    io,
    pin::{Pin, pin},
    process::ExitCode,
};

use fspy::AccessMode;
use futures_util::future::{Either, select};
use tokio::{
    fs::File,
    io::{AsyncWrite, stdout},
};

#[tokio::main]
async fn main() -> io::Result<ExitCode> {
    let mut args = args_os();
    let _ = args.next();
    assert_eq!(args.next().as_deref(), Some(OsStr::new("-o")));
    let out_path = args.next().unwrap();

    let program = args.next().unwrap();

    let (status_fut, mut acc_stream) = fspy::spy(
        program,
        Option::<&OsStr>::None,
        Option::<&OsStr>::None,
        args,
        env::vars_os(),
    )?;

    let out_file: Pin<Box<dyn AsyncWrite>> = if out_path == "-" {
        Box::pin(stdout())
    } else {
        Box::pin(File::create(out_path).await?)
    };

    let mut csv_writer = csv_async::AsyncWriter::from_writer(out_file);

    let loop_stream = async move {
        while let Some(acc) = acc_stream.next().await? {
            csv_writer
                .write_record(&[
                    acc.path,
                    match acc.access_mode {
                        AccessMode::Read => b"r",
                        AccessMode::ReadWrite => b"rw",
                        AccessMode::Write => b"w",
                    },
                    acc.caller
                ])
                .await?;
            acc_stream.bump_mut().reset();
        }
        csv_writer.flush().await?;
        io::Result::Ok(())
    };
    let status_fut = pin!(status_fut);
    let loop_stream = pin!(loop_stream);

    match select(status_fut, loop_stream).await {
        Either::Right(_) => unreachable!("access stream ended before process exits"),
        Either::Left((status, _)) => {
            let status = status?;
            if let Some(code) = status.code()
                && let Ok(code) = u8::try_from(code)
            {
                Ok(code.into())
            } else {
                Ok(255.into())
            }
        }
    }
}
