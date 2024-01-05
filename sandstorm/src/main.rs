use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use dust_devil_core::{
    sandstorm::{SandstormCommandType, SandstormHandshakeStatus},
    serialize::{ByteRead, ByteWrite, SmallReadList},
    socks5::AuthMethod,
    users::UserRole,
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

    for _ in 0..5 {
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

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::ListUsers {
        println!("Server responded with user list:")
    } else {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");

    SandstormCommandType::ListAuthMethods
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::ListAuthMethods {
        println!("Server responded with auth methods list:")
    } else {
        panic!("Server did not respond with ListAuthMethods!!");
    }
    let list = SmallReadList::<(AuthMethod, bool)>::read(&mut reader).await.expect("Read failed").0;
    println!("Auth methods list: {list:?}");

    println!("Turning off AuthMethod NoAuth");
    SandstormCommandType::ToggleAuthMethod
        .write(&mut writer)
        .await
        .expect("Write failed");
    AuthMethod::NoAuth.write(&mut writer).await.expect("Write failed");
    false.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::ToggleAuthMethod {
        println!("Server responded with toggle auth method:");
    } else {
        panic!("Server did not respond with ToggleAuthMethod!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("{}", if status { "Success!" } else { "Failed 💀💀" });

    SandstormCommandType::ListAuthMethods
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::ListAuthMethods {
        println!("Server responded with auth methods list:")
    } else {
        panic!("Server did not respond with ListAuthMethods!!");
    }
    let list = SmallReadList::<(AuthMethod, bool)>::read(&mut reader).await.expect("Read failed").0;
    println!("Auth methods list: {list:?}");

    SandstormCommandType::GetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::GetBufferSize {
        println!("Server responded with get buffer size:")
    } else {
        panic!("Server did not respond with GetBufferSize!!");
    }
    let bufsize = reader.read_u32().await.expect("Read failed");
    println!("The current buffer size is {bufsize}");

    println!("Setting buffer size to 1234");
    SandstormCommandType::SetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.write_u32(1234).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::SetBufferSize {
        println!("Server responded with set buffer size:")
    } else {
        panic!("Server did not respond with SetBufferSize!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("Set buffer size status: {status}");

    println!("Setting buffer size to 0");
    SandstormCommandType::SetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.write_u32(0).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::SetBufferSize {
        println!("Server responded with set buffer size:")
    } else {
        panic!("Server did not respond with SetBufferSize!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("Set buffer size status: {status}");

    SandstormCommandType::GetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp == SandstormCommandType::GetBufferSize {
        println!("Server responded with get buffer size:")
    } else {
        panic!("Server did not respond with GetBufferSize!!");
    }
    let bufsize = reader.read_u32().await.expect("Read failed");
    println!("The current buffer size is {bufsize}");
}
