use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    context::ClientContext, log_socks_authenticated_with_userpass, log_socks_unsupported_userpass_version,
    utils::chunk_reader::read_chunked_utf8_string,
};

pub async fn handle_userpass_auth<R, W>(reader: &mut R, writer: &mut W, context: &ClientContext) -> Result<bool, io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let ver = reader.read_u8().await?;

    let status;
    if ver != 1 {
        log_socks_unsupported_userpass_version!(context, ver);
        status = false;
    } else {
        let username = read_chunked_utf8_string(reader).await?;
        let password = read_chunked_utf8_string(reader).await?;

        status = context.try_login(&username, &password);
        log_socks_authenticated_with_userpass!(context, username, status);
    }

    let buf = [1u8, !status as u8];
    writer.write_all(&buf).await?;

    Ok(status)
}
