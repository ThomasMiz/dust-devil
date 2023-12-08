use tokio::{net::{TcpListener, TcpStream, TcpSocket}, io::{AsyncReadExt, AsyncWriteExt}};


#[tokio::main(flavor = "current_thread")]
async fn main() {
    let bind_address = "localhost:1080";

    let listener = match TcpListener::bind(bind_address).await {
        Ok(result) => result,
        Err(_) => {
            println!("Failed to set up socket at {bind_address}");
            return;
        },
    };

    loop {
        match listener.accept().await {
            Ok((socket, address)) => {
                println!("Accepted new connection from {}", address);
                tokio::spawn(async move {
                    process(socket).await;
                });
            },
            Err(err) => {
                println!("Error while accepting new connection: {}", err);
            },
        }
    }
}

async fn process(mut socket: TcpStream) {
    // let (mut reader, mut writer) = TcpStream::split(&mut socket);
    // tokio::io::copy(&mut reader, &mut writer).await.unwrap();

    let mut buf = Vec::<u8>::with_capacity(4096);

    loop {
        let read_result = socket.read_buf(&mut buf).await;
        match read_result {
            Err(err) => {
                println!("Failed to read from socket {:?}, {err}", socket.peer_addr());
                break;
            },
            Ok(bytes_read) => {
                println!("Read {bytes_read} bytes from {:?}: {}", socket.peer_addr(), String::from_utf8_lossy(&buf).trim());
                socket.write_all(&buf[0..bytes_read]).await.unwrap();
                buf.clear();
            },
        }
    }
}
