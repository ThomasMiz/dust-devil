use std::io::{Error, ErrorKind};

use tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError};

pub trait ToIoResult<T> {
    fn map_err_to_io(self) -> Result<T, Error>;
}

impl<T, R> ToIoResult<T> for Result<T, SendError<R>> {
    fn map_err_to_io(self) -> Result<T, Error> {
        self.map_err(|_| Error::new(ErrorKind::Other, "Response notifier closed"))
    }
}

impl<T> ToIoResult<T> for Result<T, RecvError> {
    fn map_err_to_io(self) -> Result<T, Error> {
        self.map_err(|_| Error::new(ErrorKind::Other, "Oneshot receiver didn't receive any value"))
    }
}
