use std::{io, net::Shutdown, str::from_utf8};

use tokio::net::UnixDatagram;


#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let (a, b) = UnixDatagram::pair()?;
    a.send(b"zzzz").await?;
    // a.shutdown(Shutdown::Both)?;
    drop(a);

    let mut buf = Vec::<u8>::new();
    b.recv_buf(&mut buf).await?;
    dbg!(from_utf8(&buf).unwrap());
    println!("zzzz");
    dbg!(b.recv_buf(&mut buf).await?);
    Ok(())
}
