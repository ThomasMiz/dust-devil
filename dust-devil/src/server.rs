use std::{future::poll_fn, io, net::SocketAddr, sync::Arc};

use dust_devil_core::{
    logging::LogEvent,
    users::{UserRole, DEFAULT_USER_PASSWORD, DEFAULT_USER_USERNAME},
};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
};

use crate::{
    args::StartupArguments,
    context::{ClientContext, ServerState},
    socks5,
    users::UserManager,
};

async fn accept_from_any(listeners: &Vec<TcpListener>) -> Result<(TcpStream, SocketAddr), (&TcpListener, io::Error)> {
    poll_fn(|cx| {
        for l in listeners {
            let poll_status = l.poll_accept(cx);
            if let std::task::Poll::Ready(result) = poll_status {
                return std::task::Poll::Ready(match result {
                    Ok(result_ok) => Ok(result_ok),
                    Err(result_err) => Err((l, result_err)),
                });
            }
        }

        std::task::Poll::Pending
    })
    .await
}

pub async fn run_server(mut startup_args: StartupArguments) {
    let (tx, mut rx) = mpsc::channel(1024);

    let logger_task = tokio::spawn(async move {
        let mut recv_vec = Vec::with_capacity(16);
        let limit = recv_vec.capacity();
        while rx.recv_many(&mut recv_vec, limit).await != 0 {
            for event in recv_vec.iter() {
                println!("{}", event);
            }

            recv_vec.clear();
        }
    });

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
    for bind_address in startup_args.socks5_bind_sockets {
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

    let state = ServerState::new(users, startup_args.no_auth_enabled, startup_args.userpass_auth_enabled);
    let state = Arc::new(state);

    let mut client_id_counter: u64 = 1;

    loop {
        select! {
            accept_result = accept_from_any(&listeners) => {
                match accept_result {
                    Ok((socket, address)) => {
                        let _ = tx.send(LogEvent::NewClientConnectionAccepted(client_id_counter, address)).await;
                        let client_context = ClientContext::create(client_id_counter, &state, &tx);
                        client_id_counter += 1;
                        tokio::spawn(async move {
                            socks5::handle_socks5(socket, client_context).await;
                        });
                    },
                    Err((listener, err)) => {
                        let _ = tx.send(LogEvent::ClientConnectionAcceptFailed(listener.local_addr().ok(), err)).await;
                    },
                }
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    let _ = tx.send(LogEvent::SavingUsersToFile(startup_args.users_file.clone())).await;
    let save_to_file_result = state.users().save_to_file(&startup_args.users_file).await;
    let _ = tx
        .send(LogEvent::UsersSavedToFile(startup_args.users_file, save_to_file_result))
        .await;

    drop(tx);
    if let Err(je) = logger_task.await {
        eprintln!("Error while joining logger task: {je}");
    }
    println!("Goodbye!");
}
