use futures_util::try_future::try_join;
use snafu::{Snafu, ResultExt};
use std::net::SocketAddr;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncReadExt;

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
        println!("Connection accepted on listening socket");

        // when new connection received, spawn a new task
        tokio::spawn(async move {
            // in this new task, connect to target service
            let mut to_socket = TcpStream::connect(to_address)
                .await
                .expect("Failed to connect to target");
            let (mut to_socket_read, mut to_socket_write) = to_socket.split();
            let (mut from_socket_read, mut from_socket_write) = from_socket.split();

            try_join(
                from_socket_read.copy(&mut to_socket_write),
                to_socket_read.copy(&mut from_socket_write),
            ).await
            .expect("Unable to read or write to sockets");
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
