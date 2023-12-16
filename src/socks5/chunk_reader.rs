use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncReadExt};

pub async fn read_chunked_ascii_string<R>(reader: &mut R) -> Result<String, io::Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let length = reader.read_u8().await? as usize;
    let mut s = String::with_capacity(length as usize);

    unsafe {
        let buf = s.as_mut_vec();
        buf.set_len(length);
        // SAFETY: We ensure the bytes read into the string are valid UTF-8 by checking that they are ASCII

        let mut total = 0;
        while total < length {
            let more = reader.read(&mut buf[total..(length - total)]).await?;

            for c in &buf[total..(total + more)] {
                if *c < b' ' || *c > 126 {
                    return Err(ErrorKind::InvalidData.into());
                }
            }

            total += more;
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
        // SAFETY: We ensure the bytes read into the string are valid UTF-8 by checking that they are ASCII

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
