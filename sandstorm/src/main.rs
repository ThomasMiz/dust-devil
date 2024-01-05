use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use dust_devil_core::{
    sandstorm::{SandstormCommandType, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpSocket,
};

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

    for _ in 0..50000 {
        SandstormCommandType::Meow.write(&mut writer).await.expect("Write failed");
        writer.flush().await.expect("Flush failed");
        println!("Meow sent");

        let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
        if resp == SandstormCommandType::Meow {
            println!("server sent meow response byte");
        } else {
            panic!("Server didn't meow back!!");
        }

        let mut meow = [0u8; 4];
        reader.read_exact(&mut meow).await.expect("Read failed");
        if &meow == b"MEOW" {
            println!("The server meowed back 🐈‍⬛")
        } else {
            println!("The server didn't meow back properly! {meow:?}");
        }
    }
}
