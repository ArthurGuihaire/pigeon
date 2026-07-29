use std::{io::Write, path::{Path, PathBuf}, str::FromStr, time::Duration};
use crate::constants::{DATA_DIR};
use iroh::endpoint::{ConnectionError, RecvStream, SendStream};
use n0_error::{Result, StackErrorExt, StdResultExt};
use pigeon::FileHeader;
use tokio::fs::File;

use crate::common::{bind_endpoint, load_or_create_identity};

async fn confirm_write(filename: &str, sender_name: &str) -> bool {
    //first, ask for initial confirmation
    print!("Receive {filename} from {sender_name} ?");
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
    let mut file = File::create(file_path).await.expect("Failed to create file");

    send_stream.write_all(&[1]).await.anyerr()?;
    send_stream.finish().anyerr()?;

    let bytes_copied = tokio::io::copy(recv_stream, &mut file).await;
    match bytes_copied {
        Ok(bytes_copied) => {
            if bytes_copied > expected_size {
                eprintln!("Warning: more data written than expected, potentially corrupt file");
                println!("expected size: {expected_size}, actual size: {bytes_copied}")
            }
            else if bytes_copied < expected_size {
                eprintln!("Warning: less data written than expected, potentially corrupt file");
            }
            Ok(())
        }
        Err(e) => Err(e.into_any())
    }
}

pub async fn listen() -> Result<()> {
    let secret_key = load_or_create_identity(&DATA_DIR)?;
    let endpoint = bind_endpoint(secret_key).await?;

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

            // let message = recv.read_to_end(4096).await.anyerr()?;
            // let message = String::from_utf8(message).anyerr()?;
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
