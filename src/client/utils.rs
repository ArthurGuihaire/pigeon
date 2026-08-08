use iroh::Endpoint;
use arrayvec::ArrayString;
use iroh::endpoint::{Connection, ConnectionError};
use iroh::endpoint_info::{EndpointData, EndpointInfo};
use n0_error::{anyerr, Result};
use pigeon::common::{SECRET_KEY, bind_endpoint};
use std::io::Write;
use std::time::Duration;
use crate::{USE_SERVER, get_public_key};
use crate::mdns::get_endpoint_info_mdns;

pub enum DiscoveryType {
    MDNS,
    SERVER,
}

pub async fn get_endpoint_info_interactive() -> (EndpointInfo, DiscoveryType) {
    let mut buf = String::new();
    loop {
        print!("Target username: ");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_line(&mut buf).expect("IO error while reading from stdin");
        let name: ArrayString<32> = ArrayString::from(buf.trim()).unwrap();
        let info_option_mdns = get_endpoint_info_mdns(&name).await;
        if let Some(info) = info_option_mdns { return (info, DiscoveryType::MDNS) }
        //if mdns doesn't find it yet, then use the server
        if USE_SERVER.get().unwrap_or(&false).clone() {
            let result = get_public_key(&name).await;
            match result {
                Ok(key) => return (EndpointInfo::from_parts(key, EndpointData::default()), DiscoveryType::SERVER),
                Err(e) => {
                    eprintln!("Error getting public key for target: {e}");
                }
            }
        }
    }
}

pub async fn get_endpoint_info(connection: &Connection, endpoint: &Endpoint) -> Result<EndpointInfo> {
    let remote_id = connection.remote_id();
    if let Some(info) = endpoint.remote_info(remote_id).await {
        Ok(EndpointInfo::from_parts(remote_id, EndpointData::new(info.into_addrs().map(|a| a.addr().clone()).collect())))
    }
    else {
        Err(anyerr!("Cannot get remote endpoint info"))
    }
}

pub async fn create_endpoint() -> Result<Endpoint> {
    let secret_key = SECRET_KEY.get().expect("Failed to load secret key");
    let endpoint = bind_endpoint(secret_key.clone()).await?;

    Ok(endpoint)
}

pub async fn safe_wait_connection_closed(connection: &Connection) {
    let remote_id = connection.remote_id();
    let res = tokio::time::timeout(Duration::from_secs(3), async move {
        let closed = connection.closed().await;
        if !matches!(closed, ConnectionError::ApplicationClosed(_)) {
            println!("{remote_id} disconnected with an error: {closed:#}");
        }
    })
    .await;
    if res.is_err() {
        println!("{remote_id} did not disconnect within 3 seconds");
    }
}
