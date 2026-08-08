use std::path::Path;

use arrayvec::ArrayString;
use async_compression::tokio::write::ZstdEncoder;
use iroh::{Endpoint, endpoint::{SendStream, VarInt}, endpoint_info::EndpointInfo};
use n0_error::{Result, StdResultExt};
use pigeon::{FileHeader, constants::CHUNK_SIZE};
use tokio::{fs::File, io::{AsyncReadExt, AsyncWriteExt}};

use pigeon::common::PIGEON_ALPN;

async fn generate_header(path: &Path, sender_username: &ArrayString<32>) -> FileHeader {
    let filename = path.file_name().expect("Target path is not a file");
    let file = File::open(path).await.expect("Failed to open target file");
    let length = file.metadata().await.expect("Failed to get file metadata").len();
    FileHeader {
        size: length,
        filename: ArrayString::from(filename.to_str().unwrap()).unwrap(),
        sender_name: sender_username.clone(),
    }
}

pub async fn send_file_chunks(send_stream: &mut SendStream, file_path: &Path, expected_size: u64) -> Result<()> {
    let mut file = File::open(file_path).await?;
    let mut compressed_stream = ZstdEncoder::new(send_stream);
    let mut buf = [0u8; CHUNK_SIZE];
    let mut progress_bar = tqdm::pbar(Some(expected_size as usize));
    loop {
        let amt_read = file.read(&mut buf).await?;
        if amt_read == 0 { break; }
        compressed_stream.write_all(&buf[..amt_read]).await?;
        let _ = progress_bar.update(amt_read).map_err(|e| eprintln!("progress bar error: {e}"));
    }
    progress_bar.clear(false);
    compressed_stream.flush().await?;
    compressed_stream.shutdown().await.map_err(|e| { eprintln!("compressed stream returned error: {e}"); e } )?;
    Ok(())
}

pub async fn connect_and_send(endpoint: &Endpoint, target: &EndpointInfo, path: &Path, sender_username: &ArrayString<32>) -> Result<()> {
    let conn = endpoint.connect(target.clone(), PIGEON_ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await.anyerr()?;

    //send stream type 0
    send.write_all(&[0]).await.anyerr()?;

    //prepare header message
    let header = generate_header(path, sender_username).await;
    let header_message = postcard::to_allocvec(&header).unwrap();

    //send size of header
    send.write_u64(header_message.len() as u64).await.anyerr()?;
    send.flush().await.anyerr()?;

    //send header
    send.write_all(&header_message).await.anyerr()?;
    send.flush().await.anyerr()?;

    let response = recv.read_u8().await?;
    if response == 1 {
        send_file_chunks(&mut send, path, header.size).await?;

        let ack = recv.read_u8().await?;
        if ack == 2 {
            println!("File sent successfully")
        }
        else {
            eprintln!("Error: Invalid acknowledgement received. Something almost certainly went wrong.")
        }
    }

    conn.close(VarInt::from(0u32), b"");

    Ok(())
}
