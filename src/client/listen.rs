use std::{io::Write, path::{Path, PathBuf}, str::FromStr, time::Duration};
use crate::constants::{DATA_DIR};
use iroh::endpoint::{ConnectionError, RecvStream, SendStream};
use n0_error::{Result, StackErrorExt, StdResultExt};
use pigeon::FileHeader;
use tokio::fs::File;

use crate::common::{bind_endpoint, load_or_create_identity};

async fn confirm_overwrite(file_path: &Path) -> bool {
    print!("Overwrite {}? (y/N) ", file_path.display());
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        trimmed.starts_with('y') || trimmed.starts_with('Y')
    }
    else { false }
}

async fn recv_file_chunks(recv_stream: &mut RecvStream, send_stream: &mut SendStream, file_path: &Path, expected_size: u64) -> Result<()> {
    let result = File::create_new(file_path).await;
    let mut file = match result {
        Ok(file) => file,
        Err(e) => {
            eprintln!("File already exists: {e}");
            if confirm_overwrite(file_path).await {
                File::create(file_path).await.expect("Failed to create file")
            }
            else {
                send_stream.write_all(&[0]);
                return Err(format!("Not permitted to overwrite {}, exiting", file_path.display()).into());
            }
        }
    };

    send_stream.write_all(&[1]);

    let bytes_copied = tokio::io::copy(recv_stream, &mut file).await;
    match bytes_copied {
        Ok(bytes_copied) => {
            if bytes_copied > expected_size {
                eprintln!("Warning: more data written than expected, potentially corrupt file");
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

            let _ = recv_file_chunks(&mut recv, &PathBuf::from_str(&header.filename).unwrap(), header.size).await;

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
