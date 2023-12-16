use std::sync::Arc;

use tokio::{net::TcpListener, select};

mod socks5;
mod users;
mod utils;

use users::UserManager;

struct ServerState {
    users: UserManager,
}

const USERS_FILE: &str = "users.txt";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Loading users from {}...", USERS_FILE);

    let users = match users::UserManager::from_file(USERS_FILE).await {
        Ok(users) => {
            println!("Loaded {} users from file", users.count());
            users
        }
        Err(err) => {
            println!("Error while loading users file: {}", err);
            println!("WARNING: Starting up with a single admin:admin user");
            let users = UserManager::new();
            users.insert(String::from("admin"), String::from("admin"), users::UserRole::Admin);
            users
        }
    };

    let state = ServerState { users };
    let state = Arc::new(state);

    let bind_address = "localhost:1080";

    let listener = match TcpListener::bind(bind_address).await {
        Ok(result) => result,
        Err(_) => {
            println!("Failed to set up socket at {bind_address}");
            return;
        }
    };

    let mut client_id_counter: u64 = 1;

    loop {
        select! {
            accept_result = listener.accept() => {
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
            }
            _ = tokio::signal::ctrl_c() => {
                println!("Goodbye");
                break;
            },
        }
    }
}
