use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use dust_devil_core::sandstorm::SandstormHandshakeStatus;
use tokio::{net::TcpSocket, io::{AsyncWriteExt, BufWriter, AsyncReadExt, BufReader}};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let socket = TcpSocket::new_v4().expect("TcpSocket::new_v4() failed");

    let mut stream = socket
        .connect(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 2222)))
        .await
        .expect("Failed to connect to server");

    let (reader, writer) = stream.split();
    let mut writer = BufWriter::new(writer);
    let mut reader = BufReader::new(reader);

    let username = "drope".as_bytes();
    let password = "pedro1234".as_bytes();

    writer.write_u8(1).await.expect("Failed write");
    writer.write_u8(username.len() as u8).await.expect("Failed write");
    writer.write_all(username).await.expect("Failed write");
    writer.write_u8(password.len() as u8).await.expect("Failed write");
    writer.write_all(password).await.expect("Failed write");
    writer.flush().await.expect("Failed flush");

    let status = reader.read_u8().await.expect("Failed read");
    let status = SandstormHandshakeStatus::from_u8(status);

    match status {
        Some(status) => println!("Handshake status: {status:?}"),
        None => println!("handshake status: wtf"),
    }
}
