use std::sync::Arc;

use dust_devil_core::{
    logging::LogEvent,
    users::{UserRole, DEFAULT_USER_PASSWORD, DEFAULT_USER_USERNAME},
};
use tokio::{net::TcpListener, select};
use tokio_util::sync::CancellationToken;

use crate::{
    args::StartupArguments,
    context::{ClientContext, ServerState},
    logger::LogManager,
    printlnif, socks5,
    users::UserManager,
    utils::accept_from_any::accept_from_any,
};

pub async fn run_server(startup_args: StartupArguments) {
    let verbose = startup_args.verbose;
    printlnif!(verbose, "Starting up logger");
    let logger = LogManager::new();

    run_server_inner(startup_args, &logger).await;

    printlnif!(verbose, "Waiting for logger to shut down");
    if let Err(je) = logger.join().await {
        eprintln!("Error while joining logger task: {je}");
    }

    printlnif!(verbose, "Goodbye!");
}

async fn run_server_inner(mut startup_args: StartupArguments, logger: &LogManager) {
    let tx = logger.new_tx();
    let _ = tx.send(LogEvent::LoadingUsersFromFile(startup_args.users_file.clone())).await;

    let users = match UserManager::from_file(&startup_args.users_file).await {
        Ok(users) => {
            let _ = tx
                .send(LogEvent::UsersLoadedFromFile(
                    startup_args.users_file.clone(),
                    Ok(users.count() as u64),
                ))
                .await;
            users
        }
        Err(err) => {
            let _ = tx
                .send(LogEvent::UsersLoadedFromFile(startup_args.users_file.clone(), Err(err)))
                .await;
            UserManager::new()
        }
    };

    for (username, userdata) in startup_args.users.drain() {
        if users.insert_or_update(username.clone(), userdata.password, userdata.role) {
            let _ = tx.send(LogEvent::UserReplacedByArgs(username.clone(), userdata.role)).await;
        } else {
            let _ = tx.send(LogEvent::UserRegistered(username, userdata.role)).await;
        }
    }

    if users.is_empty() {
        let _ = tx.send(LogEvent::StartingUpWithSingleDefaultUser).await;
        users.insert(
            String::from(DEFAULT_USER_USERNAME),
            String::from(DEFAULT_USER_PASSWORD),
            UserRole::Admin,
        );
    }

    let mut listeners = Vec::new();
    printlnif!(startup_args.verbose, "Binding listener sockets");
    for bind_address in startup_args.socks5_bind_sockets {
        printlnif!(startup_args.verbose, "Binding listening socket at {bind_address}");
        match TcpListener::bind(bind_address).await {
            Ok(result) => {
                listeners.push(result);
                let _ = tx.send(LogEvent::NewListeningSocket(bind_address)).await;
            }
            Err(err) => {
                let _ = tx.send(LogEvent::FailedBindListeningSocket(bind_address, err)).await;
            }
        }
    }

    if listeners.is_empty() {
        let _ = tx.send(LogEvent::FailedBindAnySocketAborting).await;
        return;
    }

    printlnif!(startup_args.verbose, "Constructing server state");
    let state = ServerState::new(users, startup_args.no_auth_enabled, startup_args.userpass_auth_enabled);
    let state = Arc::new(state);

    let mut client_id_counter: u64 = 1;

    let client_cancel_token = CancellationToken::new();

    printlnif!(startup_args.verbose, "Entering main loop");
    loop {
        select! {
            accept_result = accept_from_any(&listeners) => {
                match accept_result {
                    Ok((socket, address)) => {
                        let _ = tx.send(LogEvent::NewClientConnectionAccepted(client_id_counter, address)).await;
                        let client_context = ClientContext::create(client_id_counter, &state, &tx);
                        client_id_counter += 1;
                        let cancel_token1 = client_cancel_token.clone();
                        tokio::spawn(async move {
                            socks5::handle_socks5(socket, client_context, cancel_token1).await;
                        });
                    },
                    Err((listener, err)) => {
                        let _ = tx.send(LogEvent::ClientConnectionAcceptFailed(listener.local_addr().ok(), err)).await;
                    },
                }
            },
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Received shutdown signal, shutting down gracefully. Signal again to shut down ungracefully.");
                break
            },
        }
    }

    printlnif!(startup_args.verbose, "Exited main loop");

    drop(listeners);
    client_cancel_token.cancel();

    let _ = tx.send(LogEvent::SavingUsersToFile(startup_args.users_file.clone())).await;
    let save_to_file_result = state.users().save_to_file(&startup_args.users_file).await;
    let _ = tx
        .send(LogEvent::UsersSavedToFile(startup_args.users_file, save_to_file_result))
        .await;
}
