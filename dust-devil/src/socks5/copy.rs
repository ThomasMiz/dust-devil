// Most of this code was copied from tokio::io::copy_bidirectional and tokio::io::copy_buf and
// adapted to work here. While using the tokio::io::copy_bidirectional worked great and without any
// issues, this server needs to collect real time metrics on how many bytes are being sent and
// received, and that wouldn't have been possible with the tokio util function, as it only gives
// the transfer metrics after returning.

use tokio::io::{AsyncBufRead, AsyncWrite};

use std::{
    future::poll_fn,
    io::Error,
    pin::Pin,
    task::{ready, Context, Poll},
};

use crate::{context::ClientContext, log_socks_destination_shutdown, log_socks_source_shutdown};

enum TransferState {
    Running,
    ShuttingDown,
    Done,
}

fn transfer_one_direction<R, W>(
    cx: &mut Context<'_>,
    state: &mut TransferState,
    reader: &mut R,
    writer: &mut W,
    context: &mut ClientContext,
    is_src_to_dst: bool,
) -> Poll<Result<(), Error>>
where
    R: AsyncBufRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut reader = Pin::new(reader);
    let mut writer = Pin::new(writer);

    loop {
        match state {
            TransferState::Running => {
                let buffer = ready!(reader.as_mut().poll_fill_buf(cx))?;
                if buffer.is_empty() {
                    if is_src_to_dst {
                        log_socks_source_shutdown!(context);
                    } else {
                        log_socks_destination_shutdown!(context);
                    }
                    ready!(writer.as_mut().poll_flush(cx))?;
                    *state = TransferState::ShuttingDown;
                    continue;
                }

                let i = ready!(writer.as_mut().poll_write(cx, buffer))?;
                if i == 0 {
                    if is_src_to_dst {
                        log_socks_source_shutdown!(context);
                    } else {
                        log_socks_destination_shutdown!(context);
                    }
                    *state = TransferState::ShuttingDown;
                    continue;
                }

                if is_src_to_dst {
                    context.register_bytes_sent(i as u64);
                } else {
                    context.register_bytes_received(i as u64);
                }

                reader.as_mut().consume(i);
            }
            TransferState::ShuttingDown => {
                ready!(writer.as_mut().poll_shutdown(cx))?;
                *state = TransferState::Done;
            }
            TransferState::Done => return Poll::Ready(Ok(())),
        }
    }
}

pub async fn copy_bidirectional<
    'a,
    R1: AsyncBufRead + Unpin + ?Sized,
    W1: AsyncWrite + Unpin + ?Sized,
    R2: AsyncBufRead + Unpin + ?Sized,
    W2: AsyncWrite + Unpin + ?Sized,
>(
    src_reader: &'a mut R1,
    src_writer: &'a mut W1,
    dst_reader: &'a mut R2,
    dst_writer: &'a mut W2,
    context: &mut ClientContext,
) -> Result<(), Error> {
    let mut src_to_dst = TransferState::Running;
    let mut dst_to_src = TransferState::Running;

    poll_fn(|cx| {
        let src_to_dst = transfer_one_direction(cx, &mut src_to_dst, src_reader, dst_writer, context, true)?;
        let dst_to_src = transfer_one_direction(cx, &mut dst_to_src, dst_reader, src_writer, context, false)?;

        // It is not a problem if ready! returns early because transfer_one_direction for the
        // other direction will keep returning TransferState::Done(count) in future calls to poll
        ready!(src_to_dst);
        ready!(dst_to_src);

        Poll::Ready(Ok(()))
    })
    .await
}
