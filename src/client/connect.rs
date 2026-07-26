use iroh::{EndpointAddr, PublicKey};
use n0_error::{Result, StdResultExt};
use pigeon::constants::DATA_DIR;

use crate::common::{PIGEON_ALPN, bind_endpoint, load_or_create_identity};

pub async fn connect(target: &PublicKey) -> Result<()> {
    let secret_key = load_or_create_identity(&DATA_DIR)?;
    let endpoint = bind_endpoint(secret_key).await?;
    let me = endpoint.id();

    endpoint.online().await;

    let addr = EndpointAddr::from_parts(target.clone(), []);
    let conn = endpoint.connect(addr, PIGEON_ALPN).await?;

    let (mut send, mut recv) = conn.open_bi().await.anyerr()?;

    let message = format!("hello from {me}");
    send.write_all(message.as_bytes()).await.anyerr()?;
    send.finish().anyerr()?;

    let reply = recv.read_to_end(4096).await.anyerr()?;
    let reply = String::from_utf8(reply).anyerr()?;
    println!("peer replied: {reply}");

    endpoint.close().await;
    Ok(())
}
