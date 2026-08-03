use std::path::Path;

use arrayvec::ArrayString;
use iroh::{EndpointAddr, PublicKey, endpoint::SendStream};
use n0_error::{Result, StdResultExt};
use pigeon::{FileHeader, common::SECRET_KEY, constants::CHUNK_SIZE};
use tokio::{fs::File, io::{AsyncReadExt, AsyncWriteExt}};

use pigeon::common::{PIGEON_ALPN, USERNAME, bind_endpoint};

async fn generate_header(path: &Path) -> FileHeader {
    let filename = path.file_name().expect("Target path is not a file");
    let file = File::open(path).await.expect("Failed to open target file");
    let length = file.metadata().await.expect("Failed to get file metadata").len();
    FileHeader {
        size: length,
        filename: ArrayString::from(filename.to_str().unwrap()).unwrap(),
        sender_name: USERNAME.get().expect("Cannot get username").clone(),
    }
}

pub async fn send_file_chunks(send_stream: &mut SendStream, file_path: &Path) -> Result<()> {
    let mut file = File::open(file_path).await?;
    let mut buffer = [0u8; CHUNK_SIZE];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        send_stream.write_all(&buffer).await.anyerr()?;
    }
    Ok(())
}

pub async fn connect_and_send(target: &PublicKey, path: &Path) -> Result<()> {
    let secret_key = SECRET_KEY.get().expect("Failed to load secret key");
    let endpoint = bind_endpoint(secret_key.clone()).await?;

    endpoint.online().await;

    let addr = EndpointAddr::from_parts(target.clone(), []);
    let conn = endpoint.connect(addr, PIGEON_ALPN).await?;

    let (mut send, mut recv) = conn.open_bi().await.anyerr()?;

    //send header message
    let header_message = postcard::to_allocvec(&generate_header(path).await).unwrap();
    send.write_all(&header_message).await.anyerr()?;
    send.flush().await.anyerr()?;

    let response = recv.read_u8().await?;
    if response == 1 {
        send_file_chunks(&mut send, path).await?;
    }

    send.finish().anyerr()?;

    endpoint.close().await;
    Ok(())
}
