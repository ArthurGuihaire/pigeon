use std::{io::Write, path::{Path, PathBuf}, str::FromStr, time::Duration};
use async_compression::tokio::write::ZstdDecoder;
use iroh::endpoint::{ConnectionError, RecvStream, SendStream};
use n0_error::{Result, StackErrorExt, StdResultExt};
use pigeon::{FileHeader, common::SECRET_KEY};
use tokio::{fs::File};
use pigeon::common::bind_endpoint;

async fn confirm_write(filename: &str, sender_name: &str) -> bool {
    //first, ask for initial confirmation
    print!("Receive {filename} from {sender_name}? (y/N) ");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    let confirmed = if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        trimmed.starts_with('y') || trimmed.starts_with('Y')
    }
    else { false };
    //if not confirmed, exit now
    if !confirmed { return false };
    //if confirmed and the file doesn't exist already, go
    if !PathBuf::from(filename).exists() { return true };
    println!("{filename} exists already");
    print!("Overwrite {filename}? (y/N) ");
    let _ = std::io::stdout().flush();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        trimmed.starts_with('y') || trimmed.starts_with('Y')
    }
    else { false }
}
async fn recv_file_chunks(recv_stream: &mut RecvStream, send_stream: &mut SendStream, file_path: &Path, expected_size: u64) -> Result<()> {
    let file = File::create(file_path).await.expect("Failed to create file");

    send_stream.write_all(&[1]).await.anyerr()?;
    send_stream.finish().anyerr()?;

    let mut decompressed_file_stream = ZstdDecoder::new(file);

    let bytes_copied = tokio::io::copy(recv_stream, &mut decompressed_file_stream).await;
    match bytes_copied {
        Ok(bytes_copied) => {
            if bytes_copied > expected_size {
                eprintln!("Warning: more data written ({bytes_copied}) than expected ({expected_size}), potentially corrupt file");
            }
            else if bytes_copied < expected_size {
                eprintln!("Warning: less data written ({bytes_copied}) than expected ({expected_size}), potentially corrupt file");
            }
            Ok(())
        }
        Err(e) => Err(e.into_any())
    }
}

pub async fn listen() -> Result<()> {
    let secret_key = SECRET_KEY.get().expect("Failed to load secret key");
    let endpoint = bind_endpoint(secret_key.clone()).await?;

    endpoint.online().await;

    while let Some(incoming) = endpoint.accept().await {
        let accepting = match incoming.accept() {
            Ok(accepting) => accepting,
            Err(err) => {
                eprintln!("incoming connection failed: {err:#}");
                continue;
            }
        };

        let conn = accepting.await?;
        let remote_id = conn.remote_id();

        tokio::spawn(async move {
            let (mut send, mut recv) = conn.accept_bi().await.anyerr()?;

            let header = recv.read_chunk(size_of::<FileHeader>()).await.anyerr()?.unwrap();
            let header: FileHeader = postcard::from_bytes(&header).anyerr()?;

            if confirm_write(&header.filename, &header.sender_name).await {
                send.write_all(&[1]).await.expect("failed to send confirmation");
                let _ = recv_file_chunks(&mut recv, &mut send, &PathBuf::from_str(&header.filename).unwrap(), header.size).await;
                println!("Finished receiving file, close with control+c");
            }
            else {
                send.write_all(&[0]).await.expect("failed to send confirmation");
                println!("Not receiving file");
            }

            let res = tokio::time::timeout(Duration::from_secs(3), async move {
                let closed = conn.closed().await;
                if !matches!(closed, ConnectionError::ApplicationClosed(_)) {
                    println!("{remote_id} disconnected with an error: {closed:#}");
                }
            })
            .await;
            if res.is_err() {
                println!("{remote_id} did not disconnect within 3 seconds");
            }

            Ok::<(), n0_error::AnyError>(())
        });
    }

    Ok(())
}
