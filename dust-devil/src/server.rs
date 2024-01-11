use std::{collections::HashMap, io, net::SocketAddr, sync::Arc};

use dust_devil_core::{
    logging::LogEventType,
    users::{UserRole, DEFAULT_USER_PASSWORD, DEFAULT_USER_USERNAME},
};
use tokio::{net::TcpListener, select};
use tokio_util::sync::CancellationToken;

use crate::{
    args::StartupArguments,
    context::{ClientContext, SandstormContext, ServerState},
    logger::{LogManager, LogSender},
    messaging::MessageType,
    printlnif, sandstorm, socks5,
    users::{UserData, UserManager},
    utils::accept_from_any::accept_from_any,
};

const MESSAGING_CHANNEL_SIZE: usize = 8;

pub async fn run_server(startup_args: StartupArguments) {
    let verbose = startup_args.verbose;
    printlnif!(verbose, "Starting up logger");

    let logger = if startup_args.events_enabled {
        let lgr = LogManager::new(verbose, !startup_args.silent, startup_args.log_file.as_deref()).await;
        Some(lgr)
    } else {
        None
    };

    run_server_inner(startup_args, logger.as_ref()).await;

    if let Some(logger) = logger {
        printlnif!(verbose, "Waiting for logger to shut down");
        if let Err(je) = logger.join().await {
            eprintln!("Error while joining logger task: {je}");
        }
    }

    printlnif!(verbose, "Goodbye!");
}

macro_rules! sendif {
    ($log_sender:expr, $event:expr) => {
        if let Some(sender) = &$log_sender {
            sender.send($event);
        }
    };
}

async fn run_server_inner(startup_args: StartupArguments, logger: Option<&LogManager>) {
    let log_sender = logger.map(|l| l.new_sender());

    sendif!(log_sender, LogEventType::LoadingUsersFromFile(startup_args.users_file.clone()));

    let users = create_user_manager(&startup_args.users_file, startup_args.users, &log_sender).await;

    let mut socks_listeners = bind_socks_sockets(startup_args.verbose, startup_args.socks5_bind_sockets, &log_sender).await;
    if socks_listeners.is_empty() {
        sendif!(log_sender, LogEventType::FailedBindAnySocketAborting);
        return;
    }

    let mut sandstorm_listeners = bind_sandstorm_sockets(startup_args.verbose, startup_args.sandstorm_bind_sockets, &log_sender).await;

    let (message_sender, mut message_receiver) = tokio::sync::mpsc::channel(MESSAGING_CHANNEL_SIZE);

    printlnif!(startup_args.verbose, "Constructing server state");
    let state = Arc::new(ServerState::new(
        startup_args.verbose,
        users,
        startup_args.no_auth_enabled,
        startup_args.userpass_auth_enabled,
        startup_args.buffer_size,
        message_sender,
    ));

    let mut client_id_counter: u64 = 1;
    let mut manager_id_counter: u64 = 1;

    let client_cancel_token = CancellationToken::new();
    let manager_cancel_token = CancellationToken::new();

    printlnif!(startup_args.verbose, "Entering main loop");

    loop {
        select! {
            accept_result = accept_from_any(&socks_listeners) => {
                match accept_result {
                    Ok((socket, address)) => {
                        sendif!(log_sender, LogEventType::NewClientConnectionAccepted(client_id_counter, address));
                        let client_context = ClientContext::create(client_id_counter, &state, logger.map(|l| l.new_sender()));
                        client_id_counter += 1;
                        let cancel_token1 = client_cancel_token.clone();
                        tokio::spawn(async move {
                            socks5::handle_socks5(socket, client_context, cancel_token1).await;
                        });
                    },
                    Err((listener, err)) => {
                        sendif!(log_sender, LogEventType::ClientConnectionAcceptFailed(listener.local_addr().ok(), err));
                    },
                }
            },
            accept_result = accept_from_any(&sandstorm_listeners) => {
                match accept_result {
                    Ok((socket, address)) => {
                        sendif!(log_sender, LogEventType::NewSandstormConnectionAccepted(manager_id_counter, address));
                        let sandstorm_context = SandstormContext::create(manager_id_counter, &state, logger.map(|l| l.new_sender()));
                        manager_id_counter += 1;
                        let cancel_token1 = manager_cancel_token.clone();
                        tokio::spawn(async move {
                            sandstorm::handle_sandstorm(socket, sandstorm_context, cancel_token1).await;
                        });
                    },
                    Err((listener, err)) => {
                        sendif!(log_sender, LogEventType::ClientConnectionAcceptFailed(listener.local_addr().ok(), err));
                    },
                }
            }
            message = message_receiver.recv() => {
                let message = message.expect("Message channel closed before server loop!");
                // `message` should never be `None`, as we still have a reference to the ServerState through the `state` variable,
                // so the sender remains alive.

                match message {
                    MessageType::ShutdownRequest(result_notifier) => {
                        let _ = result_notifier.send(());
                        eprintln!("Received shutdown request from monitoring connection, shutting down.");
                        break;
                    }
                    MessageType::ListSocks5Sockets(result_notifier) => {
                        let _ = result_notifier.send(socks_listeners.iter().filter_map(|l| l.local_addr().ok()).collect());
                    }
                    MessageType::AddSocks5Socket(socket_address, result_notifier) => match TcpListener::bind(socket_address).await {
                        Ok(result) => {
                            socks_listeners.push(result);
                            sendif!(log_sender, LogEventType::NewSocks5Socket(socket_address));
                            let _ = result_notifier.send(Ok(()));
                        }
                        Err(err) => {
                            let err2 = io::Error::new(err.kind(), err.to_string());
                            sendif!(log_sender, LogEventType::FailedBindSocks5Socket(socket_address, err));
                            let _ = result_notifier.send(Err(err2));
                        }
                    },
                    MessageType::RemoveSocks5Socket(socket_address, result_notifier) => {
                        let maybe_listener_index = socks_listeners
                            .iter()
                            .enumerate()
                            .find(|(_, l)| l.local_addr().is_ok_and(|a| a == socket_address))
                            .map(|(i, _)| i);

                        if let Some(listener_index) = maybe_listener_index {
                            socks_listeners.swap_remove(listener_index);
                            sendif!(log_sender, LogEventType::RemovedSocks5Socket(socket_address));
                        }

                        let _ = result_notifier.send(maybe_listener_index.is_some());
                    }
                    MessageType::ListSandstormSockets(result_notifier) => {
                        let _ = result_notifier.send(sandstorm_listeners.iter().filter_map(|l| l.local_addr().ok()).collect());
                    }
                    MessageType::AddSandstormSocket(socket_address, result_notifier) => match TcpListener::bind(socket_address).await {
                        Ok(result) => {
                            sandstorm_listeners.push(result);
                            sendif!(log_sender, LogEventType::NewSandstormSocket(socket_address));
                            let _ = result_notifier.send(Ok(()));
                        }
                        Err(err) => {
                            let err2 = io::Error::new(err.kind(), err.to_string());
                            sendif!(log_sender, LogEventType::FailedBindSandstormSocket(socket_address, err));
                            let _ = result_notifier.send(Err(err2));
                        }
                    },
                    MessageType::RemoveSandstormSocket(socket_address, result_notifier) => {
                        let maybe_listener_index = sandstorm_listeners
                            .iter()
                            .enumerate()
                            .find(|(_, l)| l.local_addr().is_ok_and(|a| a == socket_address))
                            .map(|(i, _)| i);

                        if let Some(listener_index) = maybe_listener_index {
                            sandstorm_listeners.swap_remove(listener_index);
                            sendif!(log_sender, LogEventType::RemovedSandstormSocket(socket_address));
                        }

                        let _ = result_notifier.send(maybe_listener_index.is_some());
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Received shutdown signal, shutting down gracefully. Signal again to shut down ungracefully.");
                sendif!(log_sender, LogEventType::ShutdownSignalReceived);
                break;
            },
        }
    }

    printlnif!(startup_args.verbose, "Exited main loop");

    drop(message_receiver);
    drop(sandstorm_listeners);
    drop(socks_listeners);
    manager_cancel_token.cancel();
    client_cancel_token.cancel();

    sendif!(log_sender, LogEventType::SavingUsersToFile(startup_args.users_file.clone()));
    let save_to_file_result = state.users().save_to_file(&startup_args.users_file).await;
    sendif!(
        log_sender,
        LogEventType::UsersSavedToFile(startup_args.users_file, save_to_file_result)
    );
}

async fn create_user_manager(users_file: &String, mut new_users: HashMap<String, UserData>, log_sender: &Option<LogSender>) -> UserManager {
    let users = match UserManager::from_file(users_file).await {
        Ok(users) => {
            sendif!(
                log_sender,
                LogEventType::UsersLoadedFromFile(users_file.clone(), Ok(users.count() as u64))
            );
            users
        }
        Err(err) => {
            sendif!(log_sender, LogEventType::UsersLoadedFromFile(users_file.clone(), Err(err)));
            UserManager::new()
        }
    };

    for (username, userdata) in new_users.drain() {
        if users.insert_or_update(username.clone(), userdata.password, userdata.role) {
            sendif!(log_sender, LogEventType::UserReplacedByArgs(username, userdata.role));
        } else {
            sendif!(log_sender, LogEventType::UserRegisteredByArgs(username, userdata.role));
        }
    }

    if users.admin_count() == 0 {
        sendif!(
            log_sender,
            LogEventType::StartingUpWithSingleDefaultUser(format!("{DEFAULT_USER_PASSWORD}:{DEFAULT_USER_PASSWORD}"))
        );
        users.insert(
            String::from(DEFAULT_USER_USERNAME),
            String::from(DEFAULT_USER_PASSWORD),
            UserRole::Admin,
        );
    }

    users
}

async fn bind_socks_sockets(verbose: bool, addresses: Vec<SocketAddr>, log_sender: &Option<LogSender>) -> Vec<TcpListener> {
    let mut socks_listeners = Vec::new();
    printlnif!(verbose, "Binding socks listener sockets");
    for bind_address in addresses {
        printlnif!(verbose, "Binding socks listening socket at {bind_address}");
        match TcpListener::bind(bind_address).await {
            Ok(result) => {
                socks_listeners.push(result);
                sendif!(log_sender, LogEventType::NewSocks5Socket(bind_address));
            }
            Err(err) => {
                sendif!(log_sender, LogEventType::FailedBindSocks5Socket(bind_address, err));
            }
        }
    }

    socks_listeners
}

async fn bind_sandstorm_sockets(verbose: bool, addresses: Vec<SocketAddr>, log_sender: &Option<LogSender>) -> Vec<TcpListener> {
    let mut sandstorm_listeners = Vec::new();
    printlnif!(verbose, "Binding sandstorm listener sockets");
    for bind_address in addresses {
        printlnif!(verbose, "Binding sandstorm listening socket at {bind_address}");
        match TcpListener::bind(bind_address).await {
            Ok(result) => {
                sandstorm_listeners.push(result);
                sendif!(log_sender, LogEventType::NewSandstormSocket(bind_address));
            }
            Err(err) => {
                sendif!(log_sender, LogEventType::FailedBindSandstormSocket(bind_address, err));
            }
        }
    }

    sandstorm_listeners
}
