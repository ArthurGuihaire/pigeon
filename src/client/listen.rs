use std::time::Duration;
use crate::constants::DATA_DIR;
use iroh::endpoint::ConnectionError;
use n0_error::{Result, StdResultExt};

use crate::common::{bind_endpoint, load_or_create_identity};

pub async fn listen() -> Result<()> {
    let secret_key = load_or_create_identity(&DATA_DIR)?;
    let endpoint = bind_endpoint(secret_key).await?;
    let me = endpoint.id();

    endpoint.online().await;

    println!("listening as {me}");
    println!("address lookup will publish reachability for this endpoint");
    println!();
    println!("share this endpoint id with peers:");
    println!("  {me}");
    println!();
    println!("connect from another machine with:");
    println!("  cargo run --bin client -- connect --to {me}");

    while let Some(incoming) = endpoint.accept().await {
        let mut accepting = match incoming.accept() {
            Ok(accepting) => accepting,
            Err(err) => {
                eprintln!("incoming connection failed: {err:#}");
                continue;
            }
        };

        let alpn = accepting.alpn().await?;
        let conn = accepting.await?;
        let remote_id = conn.remote_id();

        tokio::spawn(async move {
            let (mut send, mut recv) = conn.accept_bi().await.anyerr()?;

            let message = recv.read_to_end(4096).await.anyerr()?;
            let message = String::from_utf8(message).anyerr()?;
            println!("received from {remote_id}: {message}");

            let reply = format!("connected to {me}");
            send.write_all(reply.as_bytes()).await.anyerr()?;
            send.finish().anyerr()?;

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
