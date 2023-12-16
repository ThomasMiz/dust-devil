use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncReadExt};

pub async fn read_chunked_utf8_string<R>(reader: &mut R) -> Result<String, io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let length = reader.read_u8().await? as usize;
    let mut s = String::with_capacity(length as usize);

    unsafe {
        let buf = s.as_mut_vec();
        buf.set_len(length);
        reader.read_exact(&mut buf[0..length]).await?;

        // SAFETY: We ensure the bytes read into the string are valid UTF-8
        if std::str::from_utf8(buf).is_err() {
            return Err(ErrorKind::InvalidData.into());
        }
    }

    Ok(s)
}

pub async fn read_domainname<R>(reader: &mut R, extra_capacity: usize) -> Result<String, io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let length = reader.read_u8().await? as usize;
    if length == 0 {
        return Err(ErrorKind::InvalidData.into());
    }

    let mut s = String::with_capacity(length as usize + extra_capacity);

    unsafe {
        let buf = s.as_mut_vec();
        buf.set_len(length);
        // SAFETY: We ensure the bytes read into the string are valid UTF-8 by checking that they
        // are graphical ASCII values, which are all valid UTF-8.

        let mut total = 0;
        while total < length {
            let more = reader.read(&mut buf[total..(length - total)]).await?;

            for c in &buf[total..(total + more)] {
                if !c.is_ascii_alphanumeric() && *c != b'-' && *c != b'.' {
                    return Err(ErrorKind::InvalidData.into());
                }
            }

            total += more;
        }
    }

    Ok(s)
}
