use core::fmt;
use std::{fmt::Write, io::Error};

use tokio::io::AsyncWrite;

use crate::{args::CommandRequest, printlnif, sandstorm::SandstormRequestManager};

const RESULT_SEPARATOR: &str = "----------";

struct UserDisplayer<'a>(&'a str);

impl<'a> fmt::Display for UserDisplayer<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            match c {
                ':' => f.write_str("\\:")?,
                c => f.write_char(c)?,
            }
        }
        Ok(())
    }
}

pub async fn handle_requests<W>(
    verbose: bool,
    silent: bool,
    requests: &Vec<CommandRequest>,
    manager: &mut SandstormRequestManager<W>,
    shutdown_writer: bool,
) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    for request in requests {
        match request {
            CommandRequest::Shutdown => {
                manager
                    .shutdown_fn(move |_result| {
                        if !silent {
                            println!("Shutdown Ok");
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::ListSocks5Sockets => {
                manager
                    .list_socks5_sockets_fn(move |result| {
                        let list = result.0;
                        if !silent {
                            println!("ListSocks5Sockets ({})", list.len());
                            for addr in list {
                                println!("{addr}");
                            }
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::AddSocks5Socket(address) => {
                let address = *address;
                manager
                    .add_socks5_socket_fn(address, move |result| {
                        if !silent {
                            match result.0 {
                                Ok(()) => println!("AddSocks5Socket {address} Ok"),
                                Err(error) => println!("AddSocks5Socket {address} Error {}: {}", error.kind(), error),
                            }
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::RemoveSocks5Socket(address) => {
                let address = *address;
                manager
                    .remove_socks5_socket_fn(address, move |result| {
                        if !silent {
                            println!("RemoveSocks5Socket {address} {:?}", result.0);
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::ListSandstormSockets => {
                manager
                    .list_sandstorm_sockets_fn(move |result| {
                        let list = result.0;
                        if !silent {
                            println!("ListSandstormSockets ({})", list.len());
                            for addr in list {
                                println!("{addr}");
                            }
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::AddSandstormSocket(address) => {
                manager
                    .add_sandstorm_socket_fn(*address, move |result| {
                        if !silent {
                            match result.0 {
                                Ok(()) => println!("AddSandstormSocket Ok"),
                                Err(error) => println!("AddSandstormSocket Error {}: {}", error.kind(), error),
                            }
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::RemoveSandstormSocket(address) => {
                let address = *address;
                manager
                    .remove_sandstorm_socket_fn(address, move |result| {
                        if !silent {
                            println!("RemoveSandstormSocket {address} {:?}", result.0);
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::ListUsers => {
                manager
                    .list_users_fn(move |result| {
                        let list = result.0;
                        if !silent {
                            println!("ListUsers ({})", list.len());
                            for (username, role) in list {
                                println!("({role}) {username}");
                            }
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::AddUser(username, password, role) => {
                let user_spec = UserDisplayer(username).to_string();
                manager
                    .add_user_fn(username, password, *role, move |result| {
                        if !silent {
                            println!("AddUser {user_spec} {result:?}");
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::UpdateUser(username, maybe_password, maybe_role) => {
                let user_spec = UserDisplayer(username).to_string();
                manager
                    .update_user_fn(username, maybe_password.as_deref(), *maybe_role, move |result| {
                        if !silent {
                            println!("UpdateUser {user_spec} {result:?}");
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::DeleteUser(username) => {
                let user_spec = UserDisplayer(username).to_string();
                manager
                    .delete_user_fn(username, move |result| {
                        println!("DeleteUser {user_spec} {result:?}");
                        println!("{RESULT_SEPARATOR}");
                    })
                    .await?;
            }
            CommandRequest::ListAuthMethods => {
                manager
                    .list_auth_methods_fn(move |result| {
                        if !silent {
                            let list = result.0;
                            println!("ListAuthMethods ({})", list.len());
                            for (auth_method, status) in list {
                                println!(
                                    "{auth_method} {}",
                                    match status {
                                        true => "enabled",
                                        false => "disabled",
                                    }
                                );
                            }
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::ToggleAuthMethod(auth_method, status) => {
                let auth_method = *auth_method;
                let status = *status;
                manager
                    .toggle_auth_method_fn(auth_method, status, move |result| {
                        if !silent {
                            println!(
                                "ToggleAuthMethod {auth_method} {status} {}",
                                match result.0 {
                                    true => "Ok",
                                    false => "Error",
                                }
                            );
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::GetMetrics => {
                manager
                    .get_metrics_fn(move |result| {
                        if !silent {
                            match result.0 {
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
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::GetBufferSize => {
                manager
                    .get_buffer_size_fn(move |result| {
                        if !silent {
                            println!("GetBufferSize {}", result.0);
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::SetBufferSize(buffer_size) => {
                manager
                    .set_buffer_size_fn(*buffer_size, move |result| {
                        if !silent {
                            println!(
                                "SetBufferSize {}",
                                match result.0 {
                                    true => "Ok",
                                    false => "Error",
                                }
                            );
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
            CommandRequest::Meow => {
                manager
                    .meow_fn(move |_result| {
                        if !silent {
                            println!("Meow");
                            println!("{RESULT_SEPARATOR}");
                        }
                    })
                    .await?;
            }
        }
    }

    match shutdown_writer {
        true => {
            printlnif!(verbose, "Flushing requests and closing connection");
            manager.shutdown_and_wait().await
        }
        false => {
            printlnif!(verbose, "Flushing and waiting for responses");
            manager.flush_and_wait().await
        }
    }
}
