use std::{io, pin::pin, process::Stdio};

use futures_util::future::{select, Either};
use tokio::process::Command;


#[tokio::main]
async fn main() -> io::Result<()> {
    // 
    let mut cmd = Command::new("C:\\Users\\branchseer\\AppData\\Local\\mise\\installs\\node\\24.1.0\\node.exe");
    cmd.args([
        // "/c",
        //  "cmd", "/c",
        // "target/debug/examples/fsacc.exe"
        // "/c", "node -e require('balabala')",
        // "/c",
       "-e", "fs.readFileSync('./dasda/xas.sh')",
        //  "type x.sh"
        // "node", "--version"
    ]);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let (child, fut) = fspy::spawn(cmd)?;

    let output_fut = child.wait_with_output();

    let output_fut = pin!(output_fut);
    let fut = pin!(fut);
    match select(output_fut, fut).await {
        Either::Left((output, fut)) => {
            dbg!(output);
            fut.await?;
        },
        Either::Right((res, fut)) => {
            dbg!(res);
            fut.await?;
        },
    }
    Ok(())
}
