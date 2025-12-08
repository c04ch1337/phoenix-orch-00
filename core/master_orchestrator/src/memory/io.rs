use std::path::Path;
use tokio::fs::{File, remove_file};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Max file size allowed for reading by default (64MB)
const DEFAULT_MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;

/// Safely creates a temporary file path in the system temp directory
/// with random UUID and timestamp to prevent predictable paths
pub async fn create_secure_temp_path(prefix: &str, extension: &str) -> io::Result<std::path::PathBuf> {
    let temp_dir = std::env::temp_dir();
    
    // Generate unique, unpredictable filename with timestamp and UUID
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let uuid = Uuid::new_v4();
    
    let filename = format!("{}_{}_{}{}",
        prefix,
        timestamp,
        uuid,
        if extension.starts_with('.') { extension.to_string() }
        else { format!(".{}", extension) }
    );
    
    let path = temp_dir.join(filename);
    
    // Log temp file creation
    tracing::debug!("Created secure temp path: {}", path.display());
    
    Ok(path)
}

/// Securely clean up a temporary file, with logging on failures
pub async fn cleanup_temp_file(path: &Path) -> io::Result<()> {
    if path.exists() {
        match remove_file(path).await {
            Ok(_) => {
                tracing::debug!("Cleaned up temporary file: {}", path.display());
                Ok(())
            },
            Err(e) => {
                tracing::warn!("Failed to clean up temporary file {}: {}", path.display(), e);
                Err(e)
            }
        }
    } else {
        Ok(())
    }
}

/// Asynchronously read a file into a UTF-8 `String`, optionally limiting the
/// number of bytes read. Includes security checks for maximum file size.
///
/// If `max_bytes` is `Some(n)`, at most `n` bytes are read from the file.
/// The content is truncated at that byte boundary if the file is larger.
/// The function assumes the file contents are valid UTF-8; invalid data will
/// result in an `io::Error` with kind `InvalidData`.
pub async fn stream_read_limited(path: &Path, max_bytes: Option<u64>) -> io::Result<String> {
    // Log file access for audit purposes
    tracing::debug!("Reading file: {}", path.display());
    
    // Check file existence first
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found: {}", path.display())
        ));
    }
    
    let mut file = File::open(path).await?;
    
    // Check file size against maximum allowed
    let file_size = file.metadata().await?.len();
    let effective_max = max_bytes.unwrap_or(DEFAULT_MAX_FILE_SIZE);
    
    if file_size > effective_max {
        tracing::warn!(
            "File too large: {} ({} bytes, max allowed: {} bytes)",
            path.display(),
            file_size,
            effective_max
        );
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("File too large: {} bytes (max allowed: {} bytes)", file_size, effective_max)
        ));
    }

    let mut buf: Vec<u8> = Vec::with_capacity(file_size.min(effective_max) as usize);
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

        let n = match file.read(&mut chunk[..to_read]).await {
            Ok(n) => n,
            Err(e) => {
                tracing::error!("Error reading file {}: {}", path.display(), e);
                return Err(e);
            }
        };
        
        if n == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[..n]);
        total_read += n as u64;
    }

    // Verify UTF-8 validity with detailed error info
    match String::from_utf8(buf) {
        Ok(content) => {
            tracing::debug!("Successfully read {} bytes from {}", total_read, path.display());
            Ok(content)
        },
        Err(e) => {
            let error_position = e.utf8_error().valid_up_to();
            tracing::error!(
                "Invalid UTF-8 in file {} at byte position {}: {}",
                path.display(),
                error_position,
                e
            );
            Err(io::Error::new(io::ErrorKind::InvalidData, format!(
                "Invalid UTF-8 at byte position {}: {}",
                error_position,
                e
            )))
        }
    }
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
