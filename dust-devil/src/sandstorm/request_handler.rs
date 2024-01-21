use std::io::{Error, ErrorKind};

use dust_devil_core::{
    sandstorm::{
        AddSandstormSocketRequest, AddSocks5SocketRequest, AddUserRequest, CurrentMetricsRequest, DeleteUserRequest,
        EventStreamConfigRequest, GetBufferSizeRequest, ListAuthMethodsRequest, ListSandstormSocketsRequest, ListSocks5SocketsRequest,
        ListUsersRequest, MeowRequest, RemoveSandstormSocketRequest, RemoveSocks5SocketRequest, SandstormCommandType, SetBufferSizeRequest,
        ShutdownRequest, ToggleAuthMethodRequest, UpdateUserRequest,
    },
    serialize::ByteRead,
};

use crate::context::SandstormContext;
use tokio::{io::AsyncRead, sync::mpsc::Sender};

use super::{error_handling::ToIoResult, messaging::ResponseNotification};

pub async fn handle_requests<R>(
    reader: &mut R,
    context: &SandstormContext,
    response_notifier: Sender<ResponseNotification>,
) -> Result<(), Error>
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
) -> Result<(), Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    match command {
        SandstormCommandType::Shutdown => {
            let _ = ShutdownRequest::read(reader).await?;
            let receiver = context.request_shutdown().await;
            response_notifier
                .send(ResponseNotification::Shutdown(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::EventStreamConfig => {
            let request = EventStreamConfigRequest::read(reader).await?;
            response_notifier
                .send(ResponseNotification::LogEventConfig(request.0))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListSocks5Sockets => {
            let _ = ListSocks5SocketsRequest::read(reader).await?;
            let receiver = context.list_socks5_sockets().await;
            response_notifier
                .send(ResponseNotification::ListSocks5Sockets(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::AddSocks5Socket => {
            let request = AddSocks5SocketRequest::read(reader).await?;
            let receiver = context.add_socks5_socket(request.0).await;
            response_notifier
                .send(ResponseNotification::AddSocks5Socket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::RemoveSocks5Socket => {
            let request = RemoveSocks5SocketRequest::read(reader).await?;
            let receiver = context.remove_socks5_socket(request.0).await;
            response_notifier
                .send(ResponseNotification::RemoveSocks5Socket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListSandstormSockets => {
            let _ = ListSandstormSocketsRequest::read(reader).await?;
            let receiver = context.list_sandstorm_sockets().await;
            response_notifier
                .send(ResponseNotification::ListSandstormSockets(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::AddSandstormSocket => {
            let request = AddSandstormSocketRequest::read(reader).await?;
            let receiver = context.add_sandstorm_socket(request.0).await;
            response_notifier
                .send(ResponseNotification::AddSandstormSocket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::RemoveSandstormSocket => {
            let request = RemoveSandstormSocketRequest::read(reader).await?;
            let receiver = context.remove_sandstorm_socket(request.0).await;
            response_notifier
                .send(ResponseNotification::RemoveSandstormSocket(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListUsers => {
            let _ = ListUsersRequest::read(reader).await?;
            let snapshot = context.get_users_snapshot();
            response_notifier
                .send(ResponseNotification::ListUsers(snapshot))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::AddUser => {
            let request = AddUserRequest::read(reader).await?;
            let result = context.add_user(request.0, request.1, request.2);
            response_notifier
                .send(ResponseNotification::AddUser(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::UpdateUser => {
            let request = UpdateUserRequest::read(reader).await?;
            let result = context.update_user(request.0, request.1, request.2);
            response_notifier
                .send(ResponseNotification::UpdateUser(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::DeleteUser => {
            let request = DeleteUserRequest::read(reader).await?;
            let result = context.delete_user(request.0);
            response_notifier
                .send(ResponseNotification::DeleteUser(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ListAuthMethods => {
            let _ = ListAuthMethodsRequest::read(reader).await?;
            let auth_methods = context.get_auth_methods();
            response_notifier
                .send(ResponseNotification::ListAuthMethods(auth_methods))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::ToggleAuthMethod => {
            let request = ToggleAuthMethodRequest::read(reader).await?;
            let result = context.toggle_auth_method(request.0, request.1);
            response_notifier
                .send(ResponseNotification::ToggleAuthMethod(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::RequestCurrentMetrics => {
            let _ = CurrentMetricsRequest::read(reader).await?;
            let receiver = context.request_metrics().await;
            response_notifier
                .send(ResponseNotification::RequestCurrentMetrics(receiver))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::GetBufferSize => {
            let _ = GetBufferSizeRequest::read(reader).await?;
            let buffer_size = context.get_buffer_size();
            response_notifier
                .send(ResponseNotification::GetBufferSize(buffer_size))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::SetBufferSize => {
            let request = SetBufferSizeRequest::read(reader).await?;
            let result = context.set_buffer_size(request.0);
            response_notifier
                .send(ResponseNotification::SetBufferSize(result))
                .await
                .map_err_to_io()?;
        }
        SandstormCommandType::Meow => {
            let _ = MeowRequest::read(reader).await?;
            response_notifier.send(ResponseNotification::Meow).await.map_err_to_io()?;
        }
        c => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("Unsupported or invalid sandstorm command {c:?}"),
            ));
        }
    }

    Ok(())
}
