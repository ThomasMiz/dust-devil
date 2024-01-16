use std::net::SocketAddr;

use dust_devil_core::{
    sandstorm::{SandstormHandshakeRef, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite},
};
use tokio::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpSocket, TcpStream},
    try_join,
};

use crate::{args::StartupArguments, printlnif, requests::write_all_requests_and_shutdown, responses::print_all_responses};

const WRITE_BUFFER_SIZE: usize = 0x2000;
const READ_BUFFER_SIZE: usize = 0x2000;

pub async fn run_client(startup_args: StartupArguments) {
    let verbose = startup_args.verbose;

    if let Err(error) = run_client_inner(startup_args).await {
        eprintln!("Client finished unsuccessfully with error: {error}");
    }

    printlnif!(verbose, "Goodbye!");
}

async fn run_client_inner(startup_args: StartupArguments) -> Result<(), io::Error> {
    let mut socket = match connect(startup_args.verbose, startup_args.server_address).await {
        Ok((sock, addr)) => {
            printlnif!(!startup_args.silent, "Connected to {addr}");
            sock
        }
        Err(error) => {
            eprintln!("Failed to connect to server: {error}");
            return Ok(());
        }
    };

    let (mut read_half, write_half) = socket.split();
    let mut writer_buf = BufWriter::with_capacity(WRITE_BUFFER_SIZE, write_half);
    let writer = &mut writer_buf;

    let handshake_status = handshake(
        startup_args.verbose,
        startup_args.silent,
        &startup_args.login_credentials.0,
        &startup_args.login_credentials.1,
        writer,
        &mut read_half,
    )
    .await?;

    if !handshake_status {
        return Ok(());
    }

    let mut reader_buf = BufReader::with_capacity(READ_BUFFER_SIZE, read_half);
    let reader = &mut reader_buf;

    try_join!(
        write_all_requests_and_shutdown(startup_args.requests, writer),
        print_all_responses(startup_args.silent, reader),
    )?;

    Ok(())
}

async fn connect(verbose: bool, addresses: Vec<SocketAddr>) -> Result<(TcpStream, SocketAddr), io::Error> {
    let mut last_error = None;

    for address in addresses {
        printlnif!(verbose, "Attempting to connect to {address}");
        let new_socket_result = match address {
            SocketAddr::V4(_) => TcpSocket::new_v4(),
            SocketAddr::V6(_) => TcpSocket::new_v6(),
        };

        let socket = match new_socket_result {
            Ok(s) => s,
            Err(error) => {
                printlnif!(verbose, "Failed to bind socket! {error}");
                last_error = Some(error);
                continue;
            }
        };

        let stream = match socket.connect(address).await {
            Ok(s) => s,
            Err(error) => {
                printlnif!(verbose, "Failed to connect to {address}! {error}");
                last_error = Some(error);
                continue;
            }
        };

        return Ok((stream, address));
    }

    Err(last_error.unwrap_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No addresses specified!")))
}

async fn handshake<R, W>(
    verbose: bool,
    silent: bool,
    username: &str,
    password: &str,
    writer: &mut W,
    reader: &mut R,
) -> Result<bool, io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    printlnif!(verbose, "Sending handshake");
    SandstormHandshakeRef { username, password }.write(writer).await?;
    writer.flush().await?;

    printlnif!(verbose, "Waiting for handshake response");
    let result = SandstormHandshakeStatus::read(reader).await?;

    match result {
        SandstormHandshakeStatus::Ok => printlnif!(!silent, "Logged in successfully!"),
        SandstormHandshakeStatus::UnsupportedVersion => eprintln!("Handshake failed: Unsupported version"),
        SandstormHandshakeStatus::InvalidUsernameOrPassword => eprintln!("Handshake failed: Invalid credentials"),
        SandstormHandshakeStatus::PermissionDenied => eprintln!("Handshake failed: User doesn't have admin permissions"),
        SandstormHandshakeStatus::UnspecifiedError => eprintln!("Handshake failed with unspecified error"),
    }

    Ok(result == SandstormHandshakeStatus::Ok)
}
