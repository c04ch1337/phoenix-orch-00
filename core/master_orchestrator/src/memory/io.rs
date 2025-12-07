use std::path::Path;
use tokio::fs::File;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt};

/// Asynchronously read a file into a UTF-8 `String`, optionally limiting the
/// number of bytes read.
///
/// If `max_bytes` is `Some(n)`, at most `n` bytes are read from the file.
/// The content is truncated at that byte boundary if the file is larger.
/// The function assumes the file contents are valid UTF-8; invalid data will
/// result in an `io::Error` with kind `InvalidData`.
pub async fn stream_read_limited(path: &Path, max_bytes: Option<u64>) -> io::Result<String> {
    let mut file = File::open(path).await?;

    let mut buf: Vec<u8> = Vec::new();
    let mut total_read: u64 = 0;
    let mut chunk = [0u8; 8192];

    loop {
        let to_read = match max_bytes {
            Some(limit) => {
                if total_read >= limit {
                    break;
                }
                let remaining = (limit - total_read) as usize;
                if remaining == 0 {
                    break;
                }
                remaining.min(chunk.len())
            }
            None => chunk.len(),
        };

        let n = file.read(&mut chunk[..to_read]).await?;
        if n == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[..n]);
        total_read += n as u64;
    }

    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Asynchronously stream all data from an `AsyncRead` into a newly created file
/// at `path`. If parent directories do not exist, they are created.
///
/// This overwrites any existing file at `path`.
pub async fn stream_write_from_reader<R>(path: &Path, reader: &mut R) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    let mut file = File::create(path).await?;
    let mut buf = [0u8; 8192];

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).await?;
    }

    file.flush().await
}
