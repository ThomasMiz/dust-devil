use std::{future::poll_fn, io, net::SocketAddr, sync::Arc};

use tokio::{
    net::{TcpListener, TcpStream},
    select,
};

use crate::{
    args::StartupArguments,
    context::{ClientContext, ServerState},
    socks5,
    users::{UserManager, UserRole},
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
    println!("Loading users from {}...", startup_args.users_file);

    let users = match UserManager::from_file(&startup_args.users_file).await {
        Ok(users) => {
            println!("Loaded {} users from file", users.count());
            users
        }
        Err(err) => {
            println!("Error while loading users file: {}", err);
            UserManager::new()
        }
    };

    for (username, userdata) in startup_args.users.drain() {
        if users.insert_or_update(username.clone(), userdata.password, userdata.role) {
            println!("WARNING: Replaced user loaded from file {username} with user specified via argument");
        } else {
            println!("Registered user {username}");
        }
    }

    if users.is_empty() {
        println!("WARNING: Starting up with a single admin:admin user");
        users.insert(String::from("admin"), String::from("admin"), UserRole::Admin);
    }

    let mut listeners = Vec::new();
    for bind_address in startup_args.socks5_bind_sockets {
        match TcpListener::bind(bind_address).await {
            Ok(result) => listeners.push(result),
            Err(err) => println!("Failed to set up socket at {bind_address}: {err:?}"),
        }
    }

    if listeners.is_empty() {
        println!("Failed to bind any socket, aborting");
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
                        println!("Accepted new connection from {address}");
                        let client_context = ClientContext::create(client_id_counter, &state);
                        client_id_counter += 1;
                        tokio::spawn(async move {
                            socks5::handle_socks5(socket, client_context).await;
                        });
                    },
                    Err((listener, err)) => {
                        println!("Error while accepting new connection from socket {:?}: {err}", listener.local_addr().ok());
                    },
                }
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    println!("Saving users...");
    if let Err(err) = state.users().save_to_file(&startup_args.users_file).await {
        println!("ERROR: Failed to save users file! {err:?}");
    }

    println!("Goodbye!");
}
