use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::ServerState;

use super::chunk_reader::read_chunked_utf8_string;

pub async fn handle_userpass_auth<R, W>(reader: &mut R, writer: &mut W, state: &ServerState, client_id: u64) -> Result<bool, io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let ver = reader.read_u8().await?;

    let status;
    if ver != 1 {
        println!("Client {client_id} requested unsupported userpass auth version: {ver}");
        status = false;
    } else {
        let username = read_chunked_utf8_string(reader).await?;
        let password = read_chunked_utf8_string(reader).await?;

        status = state.users.get(&username).filter(|u| u.password() == &password).is_some();

        println!(
            "Client {client_id} authenticated {}successfully with username \"{}\" and password \"{}\"",
            if status { "" } else { "un" },
            username.replace('\\', "\\\\").replace('\"', "\\\""),
            password.replace('\\', "\\\\").replace('\"', "\\\""),
        );
    }

    let buf = [1u8, !status as u8];
    writer.write_all(&buf).await?;

    Ok(status)
}
