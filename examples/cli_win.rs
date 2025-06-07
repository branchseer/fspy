use std::{io, pin::pin, process::Stdio};

use futures_util::future::{select, Either};
use tokio::process::Command;


#[tokio::main]
async fn main() -> io::Result<()> {
    let mut cmd = Command::new("cmd");
    cmd.args([
        "/c",
         "cmd", "/c",
         "cmd", "/c",
        "echo", "hello",
    ]);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let (child, fut) = fspy::spawn(cmd)?;

    let output_fut = child.wait_with_output();

    let output_fut = pin!(output_fut);
    let fut = pin!(fut);
    match select(output_fut, fut).await {
        Either::Left((output, _)) => {
            dbg!(output);
        },
        Either::Right((res, _)) => {
            dbg!(res);
        },
    }
    Ok(())
}
