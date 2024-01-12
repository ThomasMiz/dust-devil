use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
};

use dust_devil_core::{
    sandstorm::SandstormCommandType,
    serialize::{ByteRead, SmallReadString},
    users::UserRole,
};

use crate::context::SandstormContext;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::mpsc::Sender,
};

use super::{error_handling::ToIoResult, messaging::ResponseNotification};

pub async fn handle_requests<R>(
    reader: &mut R,
    context: &SandstormContext,
    response_notifier: Sender<ResponseNotification>,
) -> Result<(), io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    loop {
        match SandstormCommandType::read(reader).await {
            Ok(command) => run_command(command, reader, context, &response_notifier).await?,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };
    }
}

async fn run_command<R>(
    command: SandstormCommandType,
    reader: &mut R,
    context: &SandstormContext,
    response_notifier: &Sender<ResponseNotification>,
) -> Result<(), io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    match command {
        SandstormCommandType::Shutdown => {
            let receiver = context.request_shutdown().await;
            response_notifier
                .send(ResponseNotification::Shutdown(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::LogEventConfig => {
            let toggle_status = bool::read(reader).await?;
            response_notifier
                .send(ResponseNotification::LogEventConfig(toggle_status))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListSocks5Sockets => {
            let receiver = context.list_socks5_sockets().await;
            response_notifier
                .send(ResponseNotification::ListSocks5Sockets(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::AddSocks5Socket => {
            let socket_address = SocketAddr::read(reader).await?;
            let receiver = context.add_socks5_socket(socket_address).await;
            response_notifier
                .send(ResponseNotification::AddSocks5Socket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::RemoveSocks5Socket => {
            let socket_address = SocketAddr::read(reader).await?;
            let receiver = context.remove_socks5_socket(socket_address).await;
            response_notifier
                .send(ResponseNotification::RemoveSocks5Socket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListSandstormSockets => {
            let receiver = context.list_sandstorm_sockets().await;
            response_notifier
                .send(ResponseNotification::ListSandstormSockets(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::AddSandstormSocket => {
            let socket_address = SocketAddr::read(reader).await?;
            let receiver = context.add_sandstorm_socket(socket_address).await;
            response_notifier
                .send(ResponseNotification::AddSandstormSocket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::RemoveSandstormSocket => {
            let socket_address = SocketAddr::read(reader).await?;
            let receiver = context.remove_sandstorm_socket(socket_address).await;
            response_notifier
                .send(ResponseNotification::RemoveSandstormSocket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListUsers => {
            let snapshot = context.get_users_snapshot();
            response_notifier
                .send(ResponseNotification::ListUsers(snapshot))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::AddUser => {
            let username = SmallReadString::read(reader).await?.0;
            let password = SmallReadString::read(reader).await?.0;
            let role = reader.read_u8().await?;

            let result = context.add_user(username, password, role);
            response_notifier
                .send(ResponseNotification::AddUser(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::UpdateUser => {
            let username = SmallReadString::read(reader).await?.0;
            let password = <Option<SmallReadString> as ByteRead>::read(reader).await?;
            let password = password.map(|s| s.0);
            let role = <Option<UserRole> as ByteRead>::read(reader).await?;

            let result = context.update_user(username, password, role);
            response_notifier
                .send(ResponseNotification::UpdateUser(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::DeleteUser => {
            let username = SmallReadString::read(reader).await?.0;
            let result = context.delete_user(username);
            response_notifier
                .send(ResponseNotification::DeleteUser(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListAuthMethods => {
            let auth_methods = context.get_auth_methods();
            response_notifier
                .send(ResponseNotification::ListAuthMethods(auth_methods))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ToggleAuthMethod => {
            let auth_method = reader.read_u8().await?;
            let state = bool::read(reader).await?;
            let result = context.toggle_auth_method(auth_method, state);
            response_notifier
                .send(ResponseNotification::ToggleAuthMethod(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::RequestCurrentMetrics => {
            let receiver = context.request_metrics().await;
            response_notifier
                .send(ResponseNotification::RequestCurrentMetrics(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::GetBufferSize => {
            let buffer_size = context.get_buffer_size();
            response_notifier
                .send(ResponseNotification::GetBufferSize(buffer_size))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::SetBufferSize => {
            let buffer_size = reader.read_u32().await?;
            let result = context.set_buffer_size(buffer_size);
            response_notifier
                .send(ResponseNotification::SetBufferSize(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::Meow => {
            response_notifier.send(ResponseNotification::Meow).await.map_err_to_io()?;
        }
        c => {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                format!("Unsupported or invalid sandstorm command {c:?}"),
            ));
        }
    }

    Ok(())
}
