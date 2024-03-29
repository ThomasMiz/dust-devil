use std::{
    future::{poll_fn, Future},
    pin::Pin,
    task::Poll,
};

use tokio::{
    select,
    sync::{broadcast, oneshot},
};

/// Runs a background future at the same time as a foreground future, until the foreground future
/// completes, at which point the background future is aborted. Note that the background future
/// should never complete, as otherwise this function will panic.
///
/// This is intended, for example, when one future controls a loading indicator while another
/// future waits for loading to complete.
pub async fn run_with_background<B, F, T>(background: B, foreground: F) -> T
where
    B: Future,
    F: Future<Output = T>,
{
    select! {
        biased;
        result = foreground => result,
        _ = background => panic!("run_with_background was called with a background future that completed"),
    }
}

/// Receives from a [`broadcast::Receiver<T>`], until either a value is received or the channel is
/// closed. In other words, receives repeatedly ignorign lagged receives.
///
/// Returns `Err(())` if the channel closed.
pub async fn recv_ignore_lagged<T: Clone>(receiver: &mut broadcast::Receiver<T>) -> Result<T, ()> {
    loop {
        match receiver.recv().await {
            Ok(result) => return Ok(result),
            Err(broadcast::error::RecvError::Closed) => return Err(()),
            Err(broadcast::error::RecvError::Lagged(_)) => {}
        }
    }
}

pub async fn recv_many_with_index<T, R, F>(vec: &mut [T], f: F) -> (Result<R, oneshot::error::RecvError>, usize)
where
    F: Fn(&mut T) -> &mut oneshot::Receiver<R>,
{
    poll_fn(|cx| {
        for (index, receiver) in vec.iter_mut().map(&f).enumerate().rev() {
            if let Poll::Ready(result) = Pin::new(receiver).poll(cx) {
                return Poll::Ready((result, index));
            }
        }

        Poll::Pending
    })
    .await
}
