use tokio::{net::TcpListener, select};

mod socks5;

#[tokio::main(flavor = "current_thread")]
async fn main() {
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
                        tokio::spawn(async move {
                            socks5::handle_socks5(socket, client_id).await;
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
