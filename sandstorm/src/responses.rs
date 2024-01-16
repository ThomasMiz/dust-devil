use std::io::ErrorKind;

use dust_devil_core::{
    sandstorm::{
        AddSandstormSocketResponse, AddSocks5SocketResponse, AddUserResponse, CurrentMetricsResponse, DeleteUserResponse,
        GetBufferSizeResponse, ListAuthMethodsResponse, ListSandstormSocketsResponse, ListSocks5SocketsResponse, ListUsersResponse,
        MeowResponse, RemoveSandstormSocketResponse, RemoveSocketResponse, RemoveSocks5SocketResponse, SandstormCommandType,
        SetBufferSizeResponse, ShutdownResponse, ToggleAuthMethodResponse, UpdateUserResponse,
    },
    serialize::ByteRead,
};
use tokio::io::{self, AsyncRead};

use crate::printlnif;

pub async fn print_all_responses<R>(silent: bool, reader: &mut R) -> Result<(), io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    loop {
        let command = match SandstormCommandType::read(reader).await {
            Ok(cmd) => cmd,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        };

        match command {
            SandstormCommandType::Shutdown => {
                ShutdownResponse::read(reader).await?;
                printlnif!(!silent, "Shutdown Ok");
            }
            SandstormCommandType::EventStreamConfig => {
                eprintln!("Received an EventStreamConfig response (this should not happen)");
            }
            SandstormCommandType::EventStream => {
                eprintln!("Received an EventStream response (this should not happen)");
            }
            SandstormCommandType::ListSocks5Sockets => {
                let list = ListSocks5SocketsResponse::read(reader).await?.0;
                if !silent {
                    println!("ListSocks5Sockets ({})", list.len());
                    for addr in list {
                        println!("{addr}");
                    }
                }
            }
            SandstormCommandType::AddSocks5Socket => {
                let result = AddSocks5SocketResponse::read(reader).await?.0;
                if !silent {
                    match result {
                        Ok(()) => println!("AddSocks5Socket Ok"),
                        Err(error) => println!("AddSocks5Socket Error {}: {}", error.kind(), error),
                    }
                }
            }
            SandstormCommandType::RemoveSocks5Socket => {
                let result = RemoveSocks5SocketResponse::read(reader).await?.0;
                printlnif!(
                    !silent,
                    "RemoveSocks5Socket {}",
                    match result {
                        RemoveSocketResponse::Ok => "Ok",
                        RemoveSocketResponse::SocketNotFound => "SocketNotFound",
                    }
                );
            }
            SandstormCommandType::ListSandstormSockets => {
                let list = ListSandstormSocketsResponse::read(reader).await?.0;
                if !silent {
                    println!("ListSandstorm5Sockets ({})", list.len());
                    for addr in list {
                        println!("{addr}");
                    }
                }
            }
            SandstormCommandType::AddSandstormSocket => {
                let result = AddSandstormSocketResponse::read(reader).await?.0;
                if !silent {
                    match result {
                        Ok(()) => println!("AddSandstormSocket Ok"),
                        Err(error) => println!("AddSandstormSocket Error {}: {}", error.kind(), error),
                    }
                }
            }
            SandstormCommandType::RemoveSandstormSocket => {
                let result = RemoveSandstormSocketResponse::read(reader).await?.0;
                printlnif!(
                    !silent,
                    "RemoveSandstormSocket {}",
                    match result {
                        RemoveSocketResponse::Ok => "Ok",
                        RemoveSocketResponse::SocketNotFound => "SocketNotFound",
                    }
                );
            }
            SandstormCommandType::ListUsers => {
                let list = ListUsersResponse::read(reader).await?.0;
                if !silent {
                    println!("ListUsers ({})", list.len());
                    for (username, role) in list {
                        println!("({role}) {username}");
                    }
                }
            }
            SandstormCommandType::AddUser => {
                let result = AddUserResponse::read(reader).await?;
                printlnif!(
                    !silent,
                    "AddUser {}",
                    match result {
                        AddUserResponse::Ok => "Ok",
                        AddUserResponse::AlreadyExists => "AlreadyExists",
                        AddUserResponse::InvalidValues => "InvalidValues",
                    }
                );
            }
            SandstormCommandType::UpdateUser => {
                let result = UpdateUserResponse::read(reader).await?;
                printlnif!(
                    !silent,
                    "UpdateUser {}",
                    match result {
                        UpdateUserResponse::Ok => "Ok",
                        UpdateUserResponse::UserNotFound => "UserNotFound",
                        UpdateUserResponse::CannotDeleteOnlyAdmin => "CannotDeleteOnlyAdmin",
                        UpdateUserResponse::NothingWasRequested => "NothingWasRequested",
                    }
                );
            }
            SandstormCommandType::DeleteUser => {
                let result = DeleteUserResponse::read(reader).await?;
                printlnif!(
                    !silent,
                    "DeleteUser {}",
                    match result {
                        DeleteUserResponse::Ok => "Ok",
                        DeleteUserResponse::UserNotFound => "UserNotFound",
                        DeleteUserResponse::CannotDeleteOnlyAdmin => "CannotDeleteOnlyAdmin",
                    }
                );
            }
            SandstormCommandType::ListAuthMethods => {
                let list = ListAuthMethodsResponse::read(reader).await?.0;
                if !silent {
                    println!("ListAuthMethods ({})", list.len());
                    for (auth_method, status) in list {
                        println!("{auth_method} {status}");
                    }
                }
            }
            SandstormCommandType::ToggleAuthMethod => {
                let result = ToggleAuthMethodResponse::read(reader).await?.0;
                printlnif!(
                    !silent,
                    "ToggleAuthMethod {}",
                    match result {
                        true => "Ok",
                        false => "Error",
                    }
                );
            }
            SandstormCommandType::RequestCurrentMetrics => {
                let result = CurrentMetricsResponse::read(reader).await?.0;
                if !silent {
                    match result {
                        Some(metrics) => {
                            println!("RequestCurrentMetrics Some");
                            println!("current client connections: {}", metrics.current_client_connections);
                            println!("historic client connections: {}", metrics.historic_client_connections);
                            println!("client bytes sent: {}", metrics.client_bytes_sent);
                            println!("client bytes received: {}", metrics.client_bytes_received);
                            println!("current sandstorm connections: {}", metrics.current_sandstorm_connections);
                            println!("historic sandstorm connections: {}", metrics.historic_sandstorm_connections);
                        }
                        None => println!("RequestCurrentMetrics None"),
                    }
                }
            }
            SandstormCommandType::GetBufferSize => {
                let result = GetBufferSizeResponse::read(reader).await?.0;
                printlnif!(!silent, "GetBufferSize {result}");
            }
            SandstormCommandType::SetBufferSize => {
                let result = SetBufferSizeResponse::read(reader).await?.0;
                printlnif!(
                    !silent,
                    "SetBufferSize {}",
                    match result {
                        true => "Ok",
                        false => "Error",
                    }
                );
            }
            SandstormCommandType::Meow => {
                MeowResponse::read(reader).await?;
                printlnif!(!silent, "Meow");
            }
        }

        printlnif!(!silent, "----------");
    }

    Ok(())
}
