use std::{
    io::{self, stdin, ErrorKind},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use dust_devil_core::{
    sandstorm::{
        AddSandstormSocketRequest, AddSandstormSocketResponse, AddSocks5SocketRequest, AddSocks5SocketResponse, AddUserRequestRef,
        AddUserResponse, CurrentMetricsRequest, CurrentMetricsResponse, DeleteUserRequestRef, DeleteUserResponse, EventStreamConfigRequest,
        EventStreamConfigResponse, EventStreamResponse, GetBufferSizeRequest, GetBufferSizeResponse, ListAuthMethodsRequest,
        ListAuthMethodsResponse, ListSandstormSocketsRequest, ListSandstormSocketsResponse, ListSocks5SocketsRequest,
        ListSocks5SocketsResponse, ListUsersRequest, ListUsersResponse, MeowRequest, MeowResponse, RemoveSandstormSocketRequest,
        RemoveSandstormSocketResponse, RemoveSocks5SocketRequest, RemoveSocks5SocketResponse, SandstormCommandType, SandstormHandshakeRef,
        SandstormHandshakeStatus, SetBufferSizeRequest, SetBufferSizeResponse, ShutdownRequest, ToggleAuthMethodRequest,
        ToggleAuthMethodResponse, UpdateUserRequestRef, UpdateUserResponse,
    },
    serialize::{ByteRead, ByteWrite},
    socks5::AuthMethod,
    users::UserRole,
};
use time::{OffsetDateTime, UtcOffset};
use tokio::{io::BufReader, net::TcpSocket};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    match run().await {
        Ok(()) => println!("Goodbye!"),
        Err(error) => println!("Error encountered! {error}"),
    }
}

async fn run() -> Result<(), io::Error> {
    let socket = TcpSocket::new_v4()?;
    let mut stream = socket.connect(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 2222))).await?;

    let (reader_half, mut writer_half) = stream.split();
    let mut reader_buf = BufReader::new(reader_half);

    let writer = &mut writer_half;
    let reader = &mut reader_buf;

    SandstormHandshakeRef::new("drope", "pedro1234").write(writer).await?;
    let status = SandstormHandshakeStatus::read(reader).await?;

    if status != SandstormHandshakeStatus::Ok {
        println!("Handshake status is not ok: {status:?}");
        return Ok(());
    }
    println!("Handshake status ok 👍");
    println!();

    for _ in 0..5 {
        MeowRequest.write(writer).await?;
        println!("Meow sent");

        let resp = SandstormCommandType::read(reader).await?;
        if resp != SandstormCommandType::Meow {
            panic!("Server didn't meow back!!");
        }

        MeowResponse::read(reader).await?;
        println!("The server meowed back 🐈‍⬛");
        println!();
    }

    ListUsersRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = ListUsersResponse::read(reader).await?.0;
    println!("User list: {list:?}");
    println!();

    ListAuthMethodsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListAuthMethods {
        panic!("Server did not respond with ListAuthMethods!!");
    }
    let list = ListAuthMethodsResponse::read(reader).await?.0;
    println!("Auth methods list: {list:?}");
    println!();

    println!("Turning off AuthMethod NoAuth");
    ToggleAuthMethodRequest(AuthMethod::NoAuth, false).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ToggleAuthMethod {
        panic!("Server did not respond with ToggleAuthMethod!!");
    }
    let status = ToggleAuthMethodResponse::read(reader).await?.0;
    println!("{}", if status { "Success!" } else { "Failed 💀💀" });
    println!();

    ListAuthMethodsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListAuthMethods {
        panic!("Server did not respond with ListAuthMethods!!");
    }
    let list =ListAuthMethodsResponse::read(reader).await?.0;
    println!("Auth methods list: {list:?}");
    println!();

    GetBufferSizeRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::GetBufferSize {
        panic!("Server did not respond with GetBufferSize!!");
    }
    let bufsize = GetBufferSizeResponse::read(reader).await?.0;
    println!("The current buffer size is {bufsize}");
    println!();

    println!("Setting buffer size to 1234");
    SetBufferSizeRequest(1234).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::SetBufferSize {
        panic!("Server did not respond with SetBufferSize!!");
    }
    let status = SetBufferSizeResponse::read(reader).await?.0;
    println!("Set buffer size status: {status}");
    println!();

    println!("Setting buffer size to 0");
    SetBufferSizeRequest(0).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::SetBufferSize {
        panic!("Server did not respond with SetBufferSize!!");
    }
    let status = SetBufferSizeResponse::read(reader).await?.0;
    println!("Set buffer size status: {status}");
    println!();

    GetBufferSizeRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::GetBufferSize {
        panic!("Server did not respond with GetBufferSize!!");
    }
    let bufsize = GetBufferSizeResponse::read(reader).await?.0;
    println!("The current buffer size is {bufsize}");
    println!();

    println!("--------------------------------------------------------------------------------");

    ListUsersRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = ListUsersResponse::read(reader).await?.0;
    println!("User list: {list:?}");
    println!();

    AddUserRequestRef("marcelo", "machelo", UserRole::Regular).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::AddUser {
        panic!("Server did not respond with AddUser!!");
    }
    let status = AddUserResponse::read(reader).await?;
    println!("Add user result: {status:?}");
    println!();

    ListUsersRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = ListUsersResponse::read(reader).await?.0;
    println!("User list: {list:?}");
    println!();

    UpdateUserRequestRef("marcelo", None, Some(UserRole::Admin)).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::UpdateUser {
        panic!("Server did not respond with UpdateUser!!");
    }
    let status = UpdateUserResponse::read(reader).await?;
    println!("Add update result: {status:?}");
    println!();

    ListUsersRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = ListUsersResponse::read(reader).await?.0;
    println!("User list: {list:?}");
    println!();

    UpdateUserRequestRef("marcelo", Some("jajas!!"), None).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::UpdateUser {
        panic!("Server did not respond with UpdateUser!!");
    }
    let status = UpdateUserResponse::read(reader).await?;
    println!("Add update result: {status:?}");
    println!();

    ListUsersRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = ListUsersResponse::read(reader).await?.0;
    println!("User list: {list:?}");
    println!();

    DeleteUserRequestRef("marcelo").write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::DeleteUser {
        panic!("Server did not respond with DeleteUser!!");
    }
    let status = DeleteUserResponse::read(reader).await?;
    println!("Add delete result: {status:?}");
    println!();

    ListUsersRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListUsers {
        panic!("Server did not respond with ListUsers!!");
    }
    let list = ListUsersResponse::read(reader).await?.0;
    println!("User list: {list:?}");
    println!();





    ListSocks5SocketsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListSocks5Sockets {
        panic!("Server did not respond with ListSocks5Sockets!!");
    }
    let list = ListSocks5SocketsResponse::read(reader).await?.0;
    println!("Socket list: {list:?}");
    println!();

    AddSocks5SocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::AddSocks5Socket {
        panic!("Server did not respond with AddSocks5Socket!!");
    }
    let result = AddSocks5SocketResponse::read(reader).await?.0;
    println!("Add socket list: {result:?}");
    println!();

    ListSocks5SocketsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListSocks5Sockets {
        panic!("Server did not respond with ListSocks5Sockets!!");
    }
    let list = ListSocks5SocketsResponse::read(reader).await?.0;
    println!("Socket list: {list:?}");
    println!();

    RemoveSocks5SocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1235))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RemoveSocks5Socket {
        panic!("Server did not respond with RemoveSocks5Socket!!");
    }
    let result = RemoveSocks5SocketResponse::read(reader).await?.0;
    println!("Remove socket list: {result:?}");
    println!();

    RemoveSocks5SocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RemoveSocks5Socket {
        panic!("Server did not respond with RemoveSocks5Socket!!");
    }
    let result = RemoveSocks5SocketResponse::read(reader).await?.0;
    println!("Remove socket list: {result:?}");
    println!();

    RemoveSocks5SocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RemoveSocks5Socket {
        panic!("Server did not respond with RemoveSocks5Socket!!");
    }
    let result = RemoveSocks5SocketResponse::read(reader).await?.0;
    println!("Remove socket list: {result:?}");
    println!();

    ListSocks5SocketsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListSocks5Sockets {
        panic!("Server did not respond with ListSocks5Sockets!!");
    }
    let list = ListSocks5SocketsResponse::read(reader).await?.0;
    println!("Socket list: {list:?}");
    println!();





    ListSandstormSocketsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListSandstormSockets {
        panic!("Server did not respond with ListSandstormSockets!!");
    }
    let list = ListSandstormSocketsResponse::read(reader).await?.0;
    println!("Sandstorm socket list: {list:?}");
    println!();

    AddSandstormSocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::AddSandstormSocket {
        panic!("Server did not respond with AddSandstormSocket!!");
    }
    let result = AddSandstormSocketResponse::read(reader).await?.0;
    println!("Add sandstorm socket list: {result:?}");
    println!();

    ListSandstormSocketsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListSandstormSockets {
        panic!("Server did not respond with ListSandstormSockets!!");
    }
    let list = ListSandstormSocketsResponse::read(reader).await?.0;
    println!("Sandstorm socket list: {list:?}");
    println!();

    RemoveSandstormSocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1235))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RemoveSandstormSocket {
        panic!("Server did not respond with RemoveSandstormSocket!!");
    }
    let result = RemoveSandstormSocketResponse::read(reader).await?.0;
    println!("Remove sandstorm socket list: {result:?}");
    println!();

    RemoveSandstormSocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RemoveSandstormSocket {
        panic!("Server did not respond with RemoveSandstormSocket!!");
    }
    let result = RemoveSandstormSocketResponse::read(reader).await?.0;
    println!("Remove sandstorm socket list: {result:?}");
    println!();

    RemoveSandstormSocketRequest(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234))).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RemoveSandstormSocket {
        panic!("Server did not respond with RemoveSandstormSocket!!");
    }
    let result = RemoveSandstormSocketResponse::read(reader).await?.0;
    println!("Remove sandstorm socket list: {result:?}");
    println!();

    ListSandstormSocketsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ListSandstormSockets {
        panic!("Server did not respond with ListSandstormSockets!!");
    }
    let list = ListSandstormSocketsResponse::read(reader).await?.0;
    println!("Sandstorm socket list: {list:?}");
    println!();




    println!("Disabling event stream (even though it's not enabled)");
    EventStreamConfigRequest(false).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::EventStreamConfig {
        panic!("Server did not respond with EventStreamConfig!!");
    }
    let response = EventStreamConfigResponse::read(reader).await?;
    println!("Event stream response: {response:?}");
    println!();




    println!("Turning on AuthMethod NoAuth");
    ToggleAuthMethodRequest(AuthMethod::NoAuth, true).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::ToggleAuthMethod {
        panic!("Server did not respond with ToggleAuthMethod!!");
    }
    let status = ToggleAuthMethodResponse::read(reader).await?.0;
    println!("Toggle auth method status: {status}");
    println!();




    println!("Getting current metrics");
    CurrentMetricsRequest.write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::RequestCurrentMetrics {
        panic!("Server did not respond with RequestCurrentMetrics!!");
    }
    let result = CurrentMetricsResponse::read(reader).await?.0;
    println!("Current metrics: {result:?}");
    println!();



    println!("Start event stream? (Y/N)");
    let mut linebuf = String::new();
    stdin().read_line(&mut linebuf).expect("Read line from stdin failed");
    if !linebuf.trim().eq_ignore_ascii_case("y") {
        println!("Shut down the server? (Y/N)");
        linebuf.clear();
        stdin().read_line(&mut linebuf).expect("Read line from stdin failed");
        if linebuf.trim().eq_ignore_ascii_case("y") {
            println!("Sending shutdown request");
            ShutdownRequest.write(writer).await?;
            let resp = match SandstormCommandType::read(reader).await {
                Ok(r) => r,
                Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            };

            if resp != SandstormCommandType::Shutdown {
                panic!("Server did not respond with Shutdown!!");
            }
        }
        return Ok(());
    }

    println!("Enabling event stream");
    EventStreamConfigRequest(true).write(writer).await?;
    let resp = SandstormCommandType::read(reader).await?;
    if resp != SandstormCommandType::EventStreamConfig {
        panic!("Server did not respond with EventStreamConfig!! {resp:?}");
    }
    let response = EventStreamConfigResponse::read(reader).await?;
    let metrics = if let EventStreamConfigResponse::Enabled(m) = response {
        m
    } else {
        panic!("Failed to enable metrics! Server responded with {response:?}!");
    };
    println!("Current metrics: {metrics:?}");
    println!();

    let utc_offset = match UtcOffset::current_local_offset() {
        Ok(offset) => offset,
        Err(_) => {
            eprintln!("Could not determine system's UTC offset, defaulting to 00:00:00");
            UtcOffset::UTC
        }
    };

    loop {
        let resp = SandstormCommandType::read(reader).await?;
        if resp != SandstormCommandType::EventStream {
            panic!("Server did not respond with EventStream!!");
        }

        let event = EventStreamResponse::read(reader).await?.0;

        let t = OffsetDateTime::from_unix_timestamp(event.timestamp)
                    .map(|t| t.to_offset(utc_offset))
                    .unwrap_or(OffsetDateTime::UNIX_EPOCH);

        println!("[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {}", t.year(), t.month() as u8, t.day(), t.hour(), t.minute(), t.second(), event.data);
    }
}
