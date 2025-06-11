use std::{
    ffi::c_void,
    fs::File,
    io::{self, Write},
    path::Path,
    pin::pin,
    process::Stdio,
};

use fspy::TracedProcess;
use futures_util::future::{Either, select};
use tokio::process::Command;
use winapi::um::{
    fileapi::CreateFileW,
    libloaderapi::{GetProcAddress, LoadLibraryA, LoadLibraryW},
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let path = Path::new("CONOUT$");
    let mut cmd = Command::new("cmd");
    cmd.args([
        // "/c",
        //  "cmd", "/c",
        // "target/debug/examples/fsacc.exe"
        "/c",
        "node -e require('./zzz/balabala1')",
        //    "-e", "require('./zzz/balabala1')",
        // "/c", "type .vscosadadde\\dasdass.json"
        // "node", "--version"
    ]);
    // cmd.stdin(Stdio::null());
    // cmd.stdout(Stdio::null());
    // cmd.stderr(Stdio::null());
    let TracedProcess {
        mut child,
        mut path_access_stream,
    } = fspy::spawn(cmd).await?;

    let mut buf = Vec::<u8>::new();
    while let Some(access) = path_access_stream.next(&mut buf).await? {
        eprintln!("{:?}", access)
    }

    let output = child.wait().await?;

    eprintln!("fspy: {}", output);
    Ok(())
}
