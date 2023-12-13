use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

const BUFFER_CAPACITY: usize = 0x2000;

#[derive(Debug)]
pub enum ProcessFileLinesError<T> {
    IO(io::Error),
    InvalidUtf8 { line_number: u32, byte_at: usize },
    LineTooLong(u32),
    Cancelled(u32, T),
}

impl<T> From<io::Error> for ProcessFileLinesError<T> {
    fn from(value: io::Error) -> Self {
        ProcessFileLinesError::IO(value)
    }
}

pub async fn process_lines_utf8<R, F, T>(reader: &mut R, mut f: F) -> Result<u32, ProcessFileLinesError<T>>
where
    R: AsyncRead + Unpin + ?Sized,
    F: FnMut(&str) -> Result<(), T>,
{
    let mut buffer = vec![0u8; BUFFER_CAPACITY].into_boxed_slice();
    let buffer_capacity = buffer.len();
    let mut buffer_length = 0;
    let mut line_start = 0;
    let mut line_number: u32 = 1;

    loop {
        let bytes_read = reader.read(&mut buffer[buffer_length..]).await?;
        if bytes_read == 0 {
            break;
        }

        for i in buffer_length..(buffer_length + bytes_read) {
            if buffer[i] == b'\n' {
                match std::str::from_utf8(&buffer[line_start..i]) {
                    Ok(s) => f(s).map_err(|t| ProcessFileLinesError::Cancelled(line_number, t))?,
                    Err(utf_error) => {
                        return Err(ProcessFileLinesError::InvalidUtf8 {
                            line_number,
                            byte_at: utf_error.valid_up_to() + 1,
                        });
                    }
                };
                line_number += 1;
                line_start = i + 1;
            }
        }

        // Note: You might think there's a risk with checking if a byte is b'\n' instead of parsing the string
        // as UTF-8 and checking for the character '\n', as maybe there's a valid multi-byte UTF-8 character
        // encoding that contains the '\n' byte within it (but isn't actually '\n').
        // This, however, is impossible thanks to a pretty neat property of UTF-8: no UTF-8 character encoding
        // is a substring of any other UTF-8 character.

        if line_start == 0 && bytes_read == buffer_capacity - buffer_length {
            return Err(ProcessFileLinesError::LineTooLong(line_number));
        }

        buffer.copy_within(line_start..(buffer_length + bytes_read), 0);
        buffer_length = buffer_length + bytes_read - line_start;
        line_start = 0;
    }

    if buffer_length != 0 {
        match std::str::from_utf8(&buffer[..buffer_length]) {
            Ok(s) => f(s).map_err(|t| ProcessFileLinesError::Cancelled(line_number, t))?,
            Err(utf_error) => {
                return Err(ProcessFileLinesError::InvalidUtf8 {
                    line_number,
                    byte_at: utf_error.valid_up_to() + 1,
                });
            }
        };
        line_number += 1;
    }

    Ok(line_number)
}
