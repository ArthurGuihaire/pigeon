use std::{io::Write, path::{Path, PathBuf}, str::FromStr, time::Duration};
use async_compression::tokio::write::ZstdDecoder;
use iroh::endpoint::{ConnectionError, RecvStream};
use n0_error::{Result, StackErrorExt, StdResultExt};
use pigeon::{FileHeader, common::SECRET_KEY};
use tokio::{fs::File, io::{AsyncWriteExt}};
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
async fn recv_file_chunks(recv_stream: &mut RecvStream, file_path: &Path, expected_size: u64) -> Result<()> {
    let file = File::create(file_path).await.expect("Failed to create file");

    let mut decompressed_file_stream = ZstdDecoder::new(file);
    // let mut decompressed_file_stream = BufWriter::new(file);
    let bytes_copied = tokio::io::copy(recv_stream, &mut decompressed_file_stream).await;
    decompressed_file_stream.flush().await?;

    // let bytes_copied = tokio::io::copy(recv_stream, &mut file).await;
    match bytes_copied {
        Ok(bytes_copied) => {
            if bytes_copied > expected_size {
                eprintln!("Warning: more data written ({bytes_copied}) than expected ({expected_size}), potentially corrupt file");
            }
            else if bytes_copied < expected_size {
                eprintln!("Warning: less data written ({bytes_copied}) than expected ({expected_size}), potentially corrupt file");
            }
            else {
                println!("File transferred successfully");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error during transfer: {e}");
            Err(e.into_any())
        }
    }
}

pub async fn listen() -> Result<()> {
    let secret_key = SECRET_KEY.get().expect("Failed to load secret key");
    let endpoint = bind_endpoint(secret_key.clone()).await?;

    endpoint.online().await;

    let accepting = loop {
       match endpoint.accept().await {
           Some(incoming) =>  {
                match incoming.accept() {
                    Ok(accepting) => break accepting,
                    Err(err) => {
                        eprintln!("incoming connection failed: {err:#}");
                        continue;
                    }
                };
            }
            None => continue
        }
    };

    let conn = accepting.await?;
    let remote_id = conn.remote_id();

    // tokio::spawn(async move {
        let (mut send, mut recv) = conn.accept_bi().await.anyerr()?;

        let header = recv.read_chunk(size_of::<FileHeader>()).await.anyerr()?.unwrap();
        let header: FileHeader = postcard::from_bytes(&header).anyerr()?;

        if confirm_write(&header.filename, &header.sender_name).await {
            send.write_all(&[1]).await.expect("failed to send confirmation");
            recv_file_chunks(&mut recv, &PathBuf::from_str(&header.filename).unwrap(), header.size).await?;
            send.write_all(&[2]).await.expect("failed to send ACK");
            send.finish().anyerr()?;
            match send.stopped().await {
                Ok(None) => {},
                Ok(Some(val)) => eprintln!("Error (stopped returned Ok(Some(val))): {val}"),
                Err(e) => eprintln!("Error (stopped returned Err(e)): {e}"),
            }
            println!("Finished receiving file");
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
    // });

    // Ok(())
}
