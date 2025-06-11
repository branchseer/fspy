use std::{ffi::c_void, io, pin::pin, process::Stdio};

use futures_util::future::{select, Either};
use tokio::process::Command;
use winapi::um::{fileapi::CreateFileW, libloaderapi::{GetProcAddress, LoadLibraryA, LoadLibraryW}};


#[tokio::main]
async fn main() -> io::Result<()> {
    // let module = unsafe { LoadLibraryA(c"kernelbase.dll".as_ptr()) };
    // let createfilew = unsafe { GetProcAddress(module, c"CreateFile2".as_ptr()) };
    // dbg!((createfilew, CreateFileW as *const c_void));
    
    // C:\\Users\\branchseer\\AppData\\Local\\mise\\installs\\node\\24.1.0\\node.exe
    let mut cmd = Command::new("C:\\Users\\branchseer\\AppData\\Local\\mise\\installs\\node\\24.1.0\\node.exe");
    cmd.args([
        // "/c",
        //  "cmd", "/c",
        // "target/debug/examples/fsacc.exe"
        // "/c", "node -e require('./zzz/balabala1')",
       "-e", "require('./zzz/balabala1')",
        // "/c", "type x.sh"
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
