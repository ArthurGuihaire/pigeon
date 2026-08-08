use std::{io::Write, path::{Path, PathBuf}, str::FromStr};
use async_compression::tokio::bufread::ZstdDecoder;
use iroh::{Endpoint, endpoint::{Connection, RecvStream, SendStream}};
use n0_error::{Result, StdResultExt, anyerr};
use pigeon::{FileHeader, constants::CHUNK_SIZE};
use tokio::{fs::File, io::{AsyncReadExt, AsyncWriteExt, BufReader}};

use crate::{mdns::exchange_usernames, utils::{get_endpoint_info, safe_wait_connection_closed}};

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
    input.clear();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        trimmed.starts_with('y') || trimmed.starts_with('Y')
    }
    else { false }
}
async fn recv_file_chunks(recv_stream: &mut RecvStream, file_path: &Path, expected_size: u64) -> Result<()> {
    let mut file = File::create(file_path).await.expect("Failed to create file");

    let mut decompressed_recv_stream = ZstdDecoder::new(BufReader::new(recv_stream));
    let mut buf = [0u8; CHUNK_SIZE];
    let mut progress_bar = tqdm::pbar(Some(expected_size as usize));
    let mut total_transferred: usize = 0;
    let bytes_copied = loop {
        let amt_read = decompressed_recv_stream.read(&mut buf).await.anyerr()?;
        if amt_read == 0 { break total_transferred }
        file.write_all(&buf[..amt_read]).await?;
        total_transferred += amt_read;
        let _ = progress_bar.update(amt_read).map_err(|e| eprintln!("progress bar error: {e}"));
    } as u64;
    file.flush().await?;
    progress_bar.clear(false);

    // let bytes_copied = tokio::io::copy(recv_stream, &mut file).await;
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

pub async fn listen(endpoint: Endpoint) -> Result<()> {
    loop {
        let conn = loop {
            match endpoint.accept().await {
                Some(incoming) =>  {
                    match incoming.accept() {
                        Ok(accepting) => break accepting.await?,
                        Err(err) => {
                            eprintln!("incoming connection failed: {err:#}");
                            continue;
                        }
                    };
                }
                None => continue
            }
        };

        let (mut send, mut recv) = conn.accept_bi().await.anyerr()?;
        println!("accepted connection");
        let stream_type = recv.read_u8().await?;
        println!("stream type {stream_type}");
        match stream_type {
            0 => return receive_file_connection(&mut send, &mut recv, &conn).await,
            1 => {
                exchange_usernames(&mut send, &mut recv, get_endpoint_info(&conn, &endpoint).await?).await?;
                safe_wait_connection_closed(&conn).await;
            },
            _ => return Err(anyerr!("unknown stream type: {}", stream_type))
        }

    }
}
pub async fn receive_file_connection(send: &mut SendStream, recv: &mut RecvStream, conn: &Connection) -> Result<()> {
    println!("receiving file");
    let header_size = recv.read_u64().await?;

    let mut buf = vec![0u8; header_size as usize];
    recv.read_exact(&mut buf).await.anyerr()?;
    // let header = recv.(size_of::<FileHeader>()).await.anyerr()?.unwrap();
    let header: FileHeader = postcard::from_bytes(&buf).anyerr()?;

    if confirm_write(&header.filename, &header.sender_name).await {
        send.write_all(&[1]).await.expect("failed to send confirmation");
        recv_file_chunks(recv, &PathBuf::from_str(&header.filename).unwrap(), header.size).await?;
        send.write_all(&[2]).await.expect("failed to send ACK");
        println!("Finished receiving file");
    }
    else {
        send.write_all(&[0]).await.expect("failed to send confirmation");
        println!("Not receiving file");
    }

    send.finish().anyerr()?;
    send.flush().await?;
    match send.stopped().await {
        Ok(None) => {},
        Ok(Some(val)) => eprintln!("Error (stopped returned Ok(Some(val))): {val}"),
        Err(e) => eprintln!("Error (stopped returned Err(e)): {e}"),
    }

    safe_wait_connection_closed(conn).await;

    Ok::<(), n0_error::AnyError>(())
}
