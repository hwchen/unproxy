use anyhow::{Context as _, Result};
use async_dup::Arc;
use async_executor::{LocalExecutor, Task};
use async_io::Async;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use futures_util::future::{try_join, TryFutureExt};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use structopt::StructOpt;

fn main() -> Result<()> {
    let opt = CliOpt::from_args();

    let from_address: SocketAddr = opt.from_address.parse()?;
    let to_address: SocketAddr = opt.to_address.parse()?;

    // set up executor
    let local_ex = LocalExecutor::new();

    local_ex.run(serve(from_address, to_address))?;

    Ok(())
}

async fn serve(from_address: SocketAddr, to_address: SocketAddr) -> Result<()> {
    // bind listener now; target service is connected when
    // new connection is received on listener.
    let from_listener = Async::<TcpListener>::bind(from_address)
        .with_context(|| format!("could not bind to tcp socket: {}", from_address))?;

    // loop over new connections
    loop {
        let (from_socket, _) = from_listener.accept()
            .await?;

        // when new connection received, spawn a new task
        Task::local(async move {
            // in this new task, connect to target service
            let to_socket = match Async::<TcpStream>::connect(to_address).await {
                Ok(s) => s,
                Err(err) => {
                    println!("Could not connect to target socket: {}", err);
                    return;
                },
            };
            let to_socket_read = Arc::new(to_socket);
            let to_socket_write = to_socket_read.clone();
            let from_socket_read = Arc::new(from_socket);
            let from_socket_write = from_socket_read.clone();

            let _ = try_join(
                copy(from_socket_read, to_socket_write),
                copy(to_socket_read, from_socket_write),
            )
            .map_ok(|(b_tx, b_rx)| println!("-> {} bytes\n<- {} bytes", b_tx, b_rx))
            .map_err(|err| println!("Unable to read or write to sockets: {}", err))
            .await;
        })
        .detach();
    }
}

/// Does normal copying with the addition that the write half of the
/// TcpStream needs to be shutdown explicitly.
/// Would normally use tokio::io::copy, but it doesn't call shutdown,
/// and it doesn't pass through the read/write halves in the result, only
/// the bytes transferred.
async fn copy(mut read_socket: Arc<Async<TcpStream>>, mut write_socket: Arc<Async<TcpStream>>) -> Result<u64>
{
    let mut buf = [0; 1024];
    let mut bytes_read = 0;

    loop {
        // read from listening side
        let n = match read_socket.read(&mut buf).await {
            Ok(n) if n == 0 => break,
            Ok(n) => n,
            Err(err) => {
                return Err(err)?;
            }
        };

        // copy to target side
        write_socket.write_all(&buf[0..n]).await?;

        bytes_read += n as u64;
    }

    // Now that the copy is done, send the shutdown signal explicitly
    write_socket.write_with(|w| w.shutdown(Shutdown::Write)).await?;

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


