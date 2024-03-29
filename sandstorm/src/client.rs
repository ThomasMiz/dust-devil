use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
};

use dust_devil_core::{
    sandstorm::{SandstormHandshakeRef, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpSocket, TcpStream},
    select,
};

use crate::{
    args::{CommandRequest, StartupArguments},
    handle_output::handle_output,
    handle_requests::handle_requests,
    printlnif,
    sandstorm::SandstormRequestManager,
    tui::{self, handle_interactive},
};

fn choose_buffer_sizes(startup_args: &StartupArguments) -> (usize, usize) {
    let read_buffer_size = match startup_args.output_logs {
        true => 0x2000,
        false => 0x1000,
    };

    let write_buffer_size = match startup_args.requests.len() {
        len if len < 16 => 0x1000,
        _ => 0x2000,
    };

    (read_buffer_size, write_buffer_size)
}

pub async fn run_client(startup_args: StartupArguments) {
    let verbose = startup_args.verbose;

    if let Err(error) = run_client_inner(startup_args).await {
        eprintln!("Client finished unsuccessfully with error: {error}");
    }

    printlnif!(verbose, "Goodbye!");
}

async fn run_client_inner(startup_args: StartupArguments) -> Result<(), Error> {
    let (read_buffer_size, write_buffer_size) = choose_buffer_sizes(&startup_args);
    printlnif!(
        startup_args.verbose,
        "Will use read buffer size of {read_buffer_size} and write buffer size of {write_buffer_size}"
    );

    let socket = match connect(startup_args.verbose, startup_args.server_address).await {
        Ok((sock, addr)) => {
            printlnif!(!startup_args.silent, "Connected to {addr}");
            sock
        }
        Err(error) => {
            eprintln!("Failed to connect to server: {error}");
            return Ok(());
        }
    };

    let (mut read_half, write_half) = socket.into_split();
    let mut writer_buf = BufWriter::with_capacity(write_buffer_size, write_half);

    let handshake_status = handshake(
        startup_args.verbose,
        startup_args.silent,
        &startup_args.login_credentials.0,
        &startup_args.login_credentials.1,
        &mut writer_buf,
        &mut read_half,
    )
    .await?;

    if !handshake_status {
        return Ok(());
    }

    let reader_buf = BufReader::with_capacity(read_buffer_size, read_half);

    let (manager, read_error_recevier) = SandstormRequestManager::new(reader_buf, writer_buf);

    let mut terminal_reset_required = false;
    let result = select! {
        biased;
        read_error_result = read_error_recevier => {

            if terminal_reset_required {
                tui::reset_terminal()?;
            }
            terminal_reset_required = false;
            match read_error_result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(Error::new(ErrorKind::ConnectionReset, "Sandstorm connection manager unexpectedly")),
            }
        },
        result = handle_connection(
            startup_args.verbose,
            startup_args.silent,
            &startup_args.requests,
            startup_args.output_logs,
            startup_args.interactive,
            &mut terminal_reset_required,
            manager
        ) => result,
    };

    if terminal_reset_required {
        tui::reset_terminal()?;
    }

    result
}

async fn connect(verbose: bool, addresses: Vec<SocketAddr>) -> Result<(TcpStream, SocketAddr), Error> {
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

    Err(last_error.unwrap_or_else(|| Error::new(ErrorKind::InvalidInput, "No addresses specified!")))
}

async fn handshake<R, W>(verbose: bool, silent: bool, username: &str, password: &str, writer: &mut W, reader: &mut R) -> Result<bool, Error>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    printlnif!(verbose, "Sending handshake");
    SandstormHandshakeRef::new(username, password).write(writer).await?;
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

async fn handle_connection<W>(
    verbose: bool,
    silent: bool,
    requests: &Vec<CommandRequest>,
    output_logs: bool,
    interactive: bool,
    terminal_reset_required: &mut bool,
    mut manager: SandstormRequestManager<W>,
) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + 'static,
{
    handle_requests(silent, requests, &mut manager).await?;

    if output_logs || interactive {
        printlnif!(verbose, "Waiting for responses");
        manager.flush_and_wait().await?;

        if output_logs {
            handle_output(verbose, manager).await
        } else {
            handle_interactive(verbose, manager, terminal_reset_required).await
        }
    } else {
        printlnif!(verbose, "Shutting down and waiting for connection to close");
        manager.shutdown_and_close().await
    }
}
