use dust_devil_core::{
    sandstorm::{
        AddSandstormSocketRequest, AddSocks5SocketRequest, AddUserRequestRef, CurrentMetricsRequest, DeleteUserRequestRef,
        GetBufferSizeRequest, ListSandstormSocketsRequest, ListSocks5SocketsRequest, ListUsersRequest, MeowRequest,
        RemoveSandstormSocketRequest, RemoveSocks5SocketRequest, SetBufferSizeRequest, ShutdownRequest, ToggleAuthMethodRequest,
        UpdateUserRequestRef,
    },
    serialize::ByteWrite,
};
use tokio::io::{self, AsyncWrite, AsyncWriteExt};

use crate::args::CommandRequest;

pub async fn write_all_requests_and_shutdown<W>(requests: Vec<CommandRequest>, writer: &mut W) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let result = write_all_requests(requests, writer).await;
    let shutdown_result = writer.shutdown().await;

    result?;
    shutdown_result
}

async fn write_all_requests<W>(requests: Vec<CommandRequest>, writer: &mut W) -> Result<(), io::Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    for req in requests {
        match req {
            CommandRequest::Shutdown => {
                ShutdownRequest.write(writer).await?;
            }
            CommandRequest::ListSocks5Sockets => {
                ListSocks5SocketsRequest.write(writer).await?;
            }
            CommandRequest::AddSocks5Socket(address) => {
                AddSocks5SocketRequest(address).write(writer).await?;
            }
            CommandRequest::RemoveSocks5Socket(address) => RemoveSocks5SocketRequest(address).write(writer).await?,
            CommandRequest::ListSandstormSockets => {
                ListSandstormSocketsRequest.write(writer).await?;
            }
            CommandRequest::AddSandstormSocket(address) => {
                AddSandstormSocketRequest(address).write(writer).await?;
            }
            CommandRequest::RemoveSandstormSocket(address) => RemoveSandstormSocketRequest(address).write(writer).await?,
            CommandRequest::ListUsers => {
                ListUsersRequest.write(writer).await?;
            }
            CommandRequest::AddUser(username, password, role) => {
                AddUserRequestRef(&username, &password, role).write(writer).await?;
            }
            CommandRequest::UpdateUser(username, maybe_password, maybe_role) => {
                UpdateUserRequestRef(&username, maybe_password.as_deref(), maybe_role)
                    .write(writer)
                    .await?;
            }
            CommandRequest::DeleteUser(username) => {
                DeleteUserRequestRef(&username).write(writer).await?;
            }
            CommandRequest::ToggleAuthMethod(auth_method, status) => {
                ToggleAuthMethodRequest(auth_method, status).write(writer).await?;
            }
            CommandRequest::GetMetrics => {
                CurrentMetricsRequest.write(writer).await?;
            }
            CommandRequest::GetBufferSize => {
                GetBufferSizeRequest.write(writer).await?;
            }
            CommandRequest::SetBufferSize(buffer_size) => {
                SetBufferSizeRequest(buffer_size).write(writer).await?;
            }
            CommandRequest::Meow => {
                MeowRequest.write(writer).await?;
            }
        }
    }

    Ok(())
}
