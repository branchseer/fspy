use std::{ffi::c_void, fs::File, io::{self, Write}, pin::pin, process::Stdio};

use futures_util::future::{select, Either};
use tokio::process::Command;
use winapi::um::{fileapi::CreateFileW, libloaderapi::{GetProcAddress, LoadLibraryA, LoadLibraryW}};


#[tokio::main]
async fn main() -> io::Result<()> {
    let mut cmd = Command::new("cmd");
    cmd.args([
        // "/c",
        //  "cmd", "/c",
        // "target/debug/examples/fsacc.exe"
        "/c", "node -e require('./zzz/balabala1')",
    //    "-e", "require('./zzz/balabala1')",
        // "/c", "type .vscosadadde\\dasdass.json"
        // "node", "--version"
    ]);
    // cmd.stdin(Stdio::null());
    // cmd.stdout(Stdio::null());
    // cmd.stderr(Stdio::null());
    let (child, fut) = fspy::spawn(cmd).await?;

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
