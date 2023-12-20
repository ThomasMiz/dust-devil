use std::{future::poll_fn, sync::Arc};

use tokio::{net::TcpListener, select};

use crate::{
    args::StartupArguments,
    socks5,
    users::{UserManager, UserRole},
};

pub struct ServerState {
    pub users: UserManager,
    pub no_auth_enabled: bool,
    pub userpass_auth_enabled: bool,
}

async fn accept_from_any(listeners: &Vec<TcpListener>) -> Result<(tokio::net::TcpStream, std::net::SocketAddr), std::io::Error> {
    poll_fn(|cx| {
        for l in listeners {
            let poll_status = l.poll_accept(cx);
            if let std::task::Poll::Ready(result) = poll_status {
                return std::task::Poll::Ready(result);
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

    let state = ServerState {
        users,
        no_auth_enabled: startup_args.no_auth_enabled,
        userpass_auth_enabled: startup_args.userpass_auth_enabled,
    };

    let state = Arc::new(state);

    let mut client_id_counter: u64 = 1;

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

    loop {
        select! {
            accept_result = accept_from_any(&listeners) => {
                match accept_result {
                    Ok((socket, address)) => {
                        println!("Accepted new connection from {}", address);
                        let client_id = client_id_counter;
                        client_id_counter += 1;
                        let state1 = Arc::clone(&state);
                        tokio::spawn(async move {
                            socks5::handle_socks5(socket, client_id, state1).await;
                        });
                    },
                    Err(err) => {
                        println!("Error while accepting new connection: {}", err);
                    },
                }
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    println!("Saving users...");
    if let Err(err) = state.users.save_to_file(&startup_args.users_file).await {
        println!("ERROR: Failed to save users file! {err:?}");
    }

    println!("Goodbye!");
}
