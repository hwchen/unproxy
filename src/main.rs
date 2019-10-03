use hyper::{Client, Request, Uri};
use hyper_tls::HttpsConnector;
use snafu::{Snafu, ResultExt, OptionExt};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opt = CliOpt::from_args();

    let from_address: SocketAddr = opt.from_address.parse()
        .context(InvalidAddress)?;
    let to_address: SocketAddr = opt.to_address.parse()
        .context(InvalidAddress)?;

    // bind listener now; target service is connected when
    // new connection is received on listener.
    let mut from_listener = TcpListener::bind(&from_address)
        .await
        .expect("could not bind to tcp socket");

    // loop over new connections
    loop {
        let (mut from_socket, _) = from_listener.accept()
            .await
            .context(TcpSocket)?;

        // when new connection received, spawn a new task
        tokio::spawn(async move {
            // in this new task, connect to target service
            let mut to_socket = TcpStream::connect(to_address)
                .await
                .expect("Failed to connect to target");
            let (mut to_socket_read, mut to_socket_write) = to_socket.split();

            let mut buf = [0u8; 1024];

            // then proxy the bytes through
            loop {
                // first half, go from -> to
                let n = match from_socket.read(&mut buf).await {
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(err) => {
                        eprintln!("Failed to read from socket: {}", err);
                        return;
                    },
                };

                if let Err(err) = to_socket_write.write_all(&buf[0..n]).await {
                    eprintln!("Failed to write to socket: {}", err);
                    return;
                }

                // second half, go to -> from
                let n = match to_socket_read.read(&mut buf).await {
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(err) => {
                        eprintln!("Failed to read from socket: {}", err);
                        return;
                    },
                };

                if let Err(err) = from_socket.write_all(&buf[0..n]).await {
                    eprintln!("Failed to write to socket: {}", err);
                    return;
                }
            }
        });
    }
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


#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Invalid Address: {}", source))]
    InvalidAddress { source: std::net::AddrParseError },
    #[snafu(display("Tcp Socket Error: {}", source))]
    TcpSocket { source: tokio::io::Error },
}
