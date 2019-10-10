use futures_util::try_future::{try_join, TryFutureExt};
use std::net::SocketAddr;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opt = CliOpt::from_args();

    let from_address: SocketAddr = opt.from_address.parse()
        .map_err(Error::InvalidAddress)?;
    let to_address: SocketAddr = opt.to_address.parse()
        .map_err(Error::InvalidAddress)?;

    // bind listener now; target service is connected when
    // new connection is received on listener.
    let mut from_listener = TcpListener::bind(&from_address)
        .await
        .expect("could not bind to tcp socket");

    // loop over new connections
    loop {
        let (mut from_socket, _) = from_listener.accept()
            .await
            .map_err(Error::TcpSocket)?;

        // when new connection received, spawn a new task
        tokio::spawn(async move {
            // in this new task, connect to target service
            let mut to_socket = match TcpStream::connect(to_address).await {
                Ok(s) => s,
                Err(err) => {
                    println!("Could not connect to target socket: {}", err);
                    return;
                },
            };
            let (mut to_socket_read, mut to_socket_write) = to_socket.split();
            let (mut from_socket_read, mut from_socket_write) = from_socket.split();

            let _ = try_join(
                copy(&mut from_socket_read, &mut to_socket_write),
                copy(&mut to_socket_read, &mut from_socket_write),
            )
            .map_ok(|(b_tx, b_rx)| println!("-> {} bytes\n<- {} bytes", b_tx, b_rx))
            .map_err(|err| println!("Unable to read or write to sockets: {}", err))
            .await;
        });
    }
}

/// Does normal copying with the addition that the write half of the
/// TcpStream needs to be shutdown explicitly.
/// Would normally use tokio::io::copy, but it doesn't call shutdown,
/// and it doesn't pass through the read/write halves in the result, only
/// the bytes transferred.
async fn copy<R, W>(read_socket: &mut R, write_socket: &mut W) -> Result<u64, Error>
    where R: AsyncReadExt + std::marker::Unpin,
          W: AsyncWriteExt + std::marker::Unpin,
{
    let mut buf = [0; 1024];
    let mut bytes_read = 0;

    loop {
        // read from listening side
        let n = match read_socket.read(&mut buf).await {
            Ok(n) if n == 0 => break,
            Ok(n) => n,
            Err(err) => {
                return Err(err).map_err(Error::TcpIo)?;
            }
        };

        // copy to target side
        write_socket.write_all(&buf[0..n]).await
            .map_err(Error::TcpIo)?;

        bytes_read += n as u64;
    }

    // Now that the copy is done, send the shutdown signal explicitly
    write_socket.shutdown().await
        .map_err(Error::TcpIo)?;

    Ok(bytes_read)
}

#[derive(Debug, StructOpt)]
#[structopt(name="unproxy")]
struct CliOpt {
    #[structopt(name="verbose", short="v", long="verbose")]
    verbose: bool,

    #[structopt(name="listen", long="listen")]
    from_address: String,

    #[structopt(name="target", long="target")]
    to_address: String,
}


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid Address: {0}")]
    InvalidAddress(#[source] std::net::AddrParseError),
    #[error("Tcp Socket Error: {0}")]
    TcpSocket(#[source] tokio::io::Error),
    #[error("Tcp Io Error: {0}")]
    TcpIo (#[source] std::io::Error),
}
