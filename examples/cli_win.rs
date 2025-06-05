use std::{io, process::Stdio};

use tokio::process::Command;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/k", "echo", "hello"]);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let mut child = fspy::spawn(cmd)?;

    let output = child.wait_with_output().await?;
    println!("{:?}", output.status.code());
    dbg!(output);

    Ok(())
}
