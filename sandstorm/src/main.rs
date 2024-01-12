use std::{net::{Ipv4Addr, SocketAddr, SocketAddrV4}, io};

use dust_devil_core::{
    sandstorm::{AddUserResponse, DeleteUserResponse, SandstormCommandType, SandstormHandshakeStatus, UpdateUserResponse},
    serialize::{ByteRead, ByteWrite, SmallReadList, SmallWriteString},
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
        None => {
            println!("handshake status: wtf");
            return;
        }
    }
    println!();

    for _ in 0..5 {
        SandstormCommandType::Meow.write(&mut writer).await.expect("Write failed");
        writer.flush().await.expect("Flush failed");
        println!("Meow sent");

        let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
        if resp != SandstormCommandType::Meow {
            panic!("Server didn't meow back!!");
        }

        let mut meow = [0u8; 4];
        reader.read_exact(&mut meow).await.expect("Read failed");
        if &meow == b"MEOW" {
            println!("The server meowed back 🐈‍⬛")
        } else {
            println!("The server didn't meow back properly! {meow:?}");
        }
        println!();
    }

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");
    println!();

    SandstormCommandType::ListAuthMethods
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListAuthMethods {
        panic!("Server did not respond with ListAuthMethods!!");
    }
    let list = SmallReadList::<(AuthMethod, bool)>::read(&mut reader).await.expect("Read failed").0;
    println!("Auth methods list: {list:?}");
    println!();

    println!("Turning off AuthMethod NoAuth");
    SandstormCommandType::ToggleAuthMethod
        .write(&mut writer)
        .await
        .expect("Write failed");
    AuthMethod::NoAuth.write(&mut writer).await.expect("Write failed");
    false.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ToggleAuthMethod {
        panic!("Server did not respond with ToggleAuthMethod!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("{}", if status { "Success!" } else { "Failed 💀💀" });
    println!();

    SandstormCommandType::ListAuthMethods
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListAuthMethods {
        panic!("Server did not respond with ListAuthMethods!!");
    }
    let list = SmallReadList::<(AuthMethod, bool)>::read(&mut reader).await.expect("Read failed").0;
    println!("Auth methods list: {list:?}");
    println!();

    SandstormCommandType::GetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::GetBufferSize {
        panic!("Server did not respond with GetBufferSize!!");
    }
    let bufsize = reader.read_u32().await.expect("Read failed");
    println!("The current buffer size is {bufsize}");
    println!();

    println!("Setting buffer size to 1234");
    SandstormCommandType::SetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.write_u32(1234).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::SetBufferSize {
        panic!("Server did not respond with SetBufferSize!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("Set buffer size status: {status}");
    println!();

    println!("Setting buffer size to 0");
    SandstormCommandType::SetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.write_u32(0).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::SetBufferSize {
        panic!("Server did not respond with SetBufferSize!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("Set buffer size status: {status}");
    println!();

    SandstormCommandType::GetBufferSize.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::GetBufferSize {
        panic!("Server did not respond with GetBufferSize!!");
    }
    let bufsize = reader.read_u32().await.expect("Read failed");
    println!("The current buffer size is {bufsize}");
    println!();

    println!("--------------------------------------------------------------------------------");

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");
    println!();

    SandstormCommandType::AddUser.write(&mut writer).await.expect("Write failed");
    (SmallWriteString("marcelo"), SmallWriteString("machelo"), UserRole::Regular)
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::AddUser {
        panic!("Server did not respond with AddUser!!");
    }
    let status = AddUserResponse::read(&mut reader).await.expect("Read failed");
    println!("Add user result: {status:?}");
    println!();

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");
    println!();

    SandstormCommandType::UpdateUser.write(&mut writer).await.expect("Write failed");
    (SmallWriteString("marcelo"), None::<SmallWriteString>, Some(UserRole::Admin))
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::UpdateUser {
        panic!("Server did not respond with UpdateUser!!");
    }
    let status = UpdateUserResponse::read(&mut reader).await.expect("Read failed");
    println!("Add update result: {status:?}");
    println!();

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");
    println!();

    SandstormCommandType::UpdateUser.write(&mut writer).await.expect("Write failed");
    (SmallWriteString("marcelo"), Some(SmallWriteString("jajas!!")), None::<UserRole>)
        .write(&mut writer)
        .await
        .expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::UpdateUser {
        panic!("Server did not respond with UpdateUser!!");
    }
    let status = UpdateUserResponse::read(&mut reader).await.expect("Read failed");
    println!("Add update result: {status:?}");
    println!();

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");
    println!();

    SandstormCommandType::DeleteUser.write(&mut writer).await.expect("Write failed");
    SmallWriteString("marcelo").write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::DeleteUser {
        panic!("Server did not respond with DeleteUser!!");
    }
    let status = DeleteUserResponse::read(&mut reader).await.expect("Read failed");
    println!("Add delete result: {status:?}");
    println!();

    SandstormCommandType::ListUsers.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = <Vec<(String, UserRole)> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("User list: {list:?}");
    println!();





    SandstormCommandType::ListSocks5Sockets.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListSocks5Sockets {
        panic!("Server did not respond with ListSocks5Sockets!!");
    }
    let list = <Vec<SocketAddr> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Socket list: {list:?}");
    println!();

    SandstormCommandType::AddSocks5Socket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::AddSocks5Socket {
        panic!("Server did not respond with AddSocks5Socket!!");
    }
    let result = <Result<(), io::Error> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Add socket list: {result:?}");
    println!();

    SandstormCommandType::ListSocks5Sockets.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListSocks5Sockets {
        panic!("Server did not respond with ListSocks5Sockets!!");
    }
    let list = <Vec<SocketAddr> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Socket list: {list:?}");
    println!();

    SandstormCommandType::RemoveSocks5Socket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1235)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::RemoveSocks5Socket {
        panic!("Server did not respond with RemoveSocks5Socket!!");
    }
    let result = reader.read_u8().await.expect("Read failed");
    println!("Remove socket list: {}", if result == 0 {"OK"} else {"Not found"});
    println!();

    SandstormCommandType::RemoveSocks5Socket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::RemoveSocks5Socket {
        panic!("Server did not respond with RemoveSocks5Socket!!");
    }
    let result = reader.read_u8().await.expect("Read failed");
    println!("Remove socket list: {}", if result == 0 {"OK"} else {"Not found"});
    println!();

    SandstormCommandType::RemoveSocks5Socket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::RemoveSocks5Socket {
        panic!("Server did not respond with RemoveSocks5Socket!!");
    }
    let result = reader.read_u8().await.expect("Read failed");
    println!("Remove socket list: {}", if result == 0 {"OK"} else {"Not found"});
    println!();

    SandstormCommandType::ListSocks5Sockets.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListSocks5Sockets {
        panic!("Server did not respond with ListSocks5Sockets!!");
    }
    let list = <Vec<SocketAddr> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Socket list: {list:?}");





    SandstormCommandType::ListSandstormSockets.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListSandstormSockets {
        panic!("Server did not respond with ListSandstormSockets!!");
    }
    let list = <Vec<SocketAddr> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Sandstorm socket list: {list:?}");
    println!();

    SandstormCommandType::AddSandstormSocket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::AddSandstormSocket {
        panic!("Server did not respond with AddSandstormSocket!!");
    }
    let result = <Result<(), io::Error> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Add sandstorm socket list: {result:?}");
    println!();

    SandstormCommandType::ListSandstormSockets.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListSandstormSockets {
        panic!("Server did not respond with ListSandstormSockets!!");
    }
    let list = <Vec<SocketAddr> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Sandstorm socket list: {list:?}");
    println!();

    SandstormCommandType::RemoveSandstormSocket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1235)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::RemoveSandstormSocket {
        panic!("Server did not respond with RemoveSandstormSocket!!");
    }
    let result = reader.read_u8().await.expect("Read failed");
    println!("Remove sandstorm socket list: {}", if result == 0 {"OK"} else {"Not found"});
    println!();

    SandstormCommandType::RemoveSandstormSocket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::RemoveSandstormSocket {
        panic!("Server did not respond with RemoveSandstormSocket!!");
    }
    let result = reader.read_u8().await.expect("Read failed");
    println!("Remove sandstorm socket list: {}", if result == 0 {"OK"} else {"Not found"});
    println!();

    SandstormCommandType::RemoveSandstormSocket.write(&mut writer).await.expect("Write failed");
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)).write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::RemoveSandstormSocket {
        panic!("Server did not respond with RemoveSandstormSocket!!");
    }
    let result = reader.read_u8().await.expect("Read failed");
    println!("Remove sandstorm socket list: {}", if result == 0 {"OK"} else {"Not found"});
    println!();

    SandstormCommandType::ListSandstormSockets.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ListSandstormSockets {
        panic!("Server did not respond with ListSandstormSockets!!");
    }
    let list = <Vec<SocketAddr> as ByteRead>::read(&mut reader).await.expect("Read failed");
    println!("Socket sandstorm list: {list:?}");
    println!();





    println!("Turning on AuthMethod NoAuth");
    SandstormCommandType::ToggleAuthMethod.write(&mut writer).await.expect("Write failed");
    AuthMethod::NoAuth.write(&mut writer).await.expect("Write failed");
    true.write(&mut writer).await.expect("Write failed");
    writer.flush().await.expect("Flush failed");
    let resp = SandstormCommandType::read(&mut reader).await.expect("Read failed");
    if resp != SandstormCommandType::ToggleAuthMethod {
        panic!("Server did not respond with ToggleAuthMethod!!");
    }
    let status = bool::read(&mut reader).await.expect("Read failed");
    println!("{}", if status { "Success!" } else { "Failed 💀💀" });
    println!();
}
