use hyper::{Client, Request, Uri};
use hyper_tls::HttpsConnector;
use snafu::{Snafu, ResultExt, OptionExt};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use tokio::net::TcpListener;
use tokio_io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opt = CliOpt::from_args();

    let from_address: SocketAddr = opt.from_address.parse()
        .context(InvalidAddress)?;
    let to_address: SocketAddr = opt.to_address.parse()
        .context(InvalidAddress)?;

    let mut listener = TcpListener::bind(&from_address)
        .await
        .expect("could not bind to tcp socket");

    loop {
        let (mut socket, _) = listener.accept()
            .await
            .context(TcpSocket)?;

        // for each socket, echo for now
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];

            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(err) => {
                        eprintln!("Failed to read from socket: {}", err);
                        return;
                    },
                };

                if let Err(err) = socket.write_all(&buf[0..n]).await {
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
