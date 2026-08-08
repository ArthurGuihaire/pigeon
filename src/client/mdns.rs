use std::{collections::HashMap, str::{self, FromStr}, sync::OnceLock};
use arrayvec::ArrayString;
use iroh::{Endpoint, endpoint::{RecvStream, SendStream, VarInt}, endpoint_info::EndpointInfo};
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
use n0_error::{AnyError, Result, StdResultExt};
use n0_future::StreamExt;
use pigeon::common::{PIGEON_ALPN, MDNS_USERNAME};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, sync::RwLock};

pub static MDNS_USERS: OnceLock<RwLock<HashMap<ArrayString<32>, EndpointInfo>>> = OnceLock::new();

pub async fn exchange_usernames(send: &mut SendStream, recv: &mut RecvStream, target: EndpointInfo) -> Result<()> {
    let read_future = async {
        let size = recv.read_u8().await? as usize;
        let mut buf = [0u8; 32];
        recv.read_exact(&mut buf[..size]).await.anyerr()?;

        let name = ArrayString::from_str(str::from_utf8(&buf[..size]).anyerr()?).anyerr()?;
        let result = MDNS_USERS.get().unwrap().write().await.insert(name, target);
        if result.is_none() { println!("Mdns found user with name {name}"); }

        Ok::<(), AnyError>(())
    };

    let write_future = async {
        let serialized = MDNS_USERNAME.get().unwrap().as_bytes();
        send.write_u8(serialized.len() as u8).await?;
        send.write_all(&serialized).await.anyerr()?;
        send.finish().anyerr()?;
        send.flush().await?;
        Ok::<(), AnyError>(())
    };

    let (read_result, write_result) = tokio::join!(read_future, write_future);
    read_result?;
    write_result?;

    return Ok(())
}

async fn subscribe_mdns_events(mdns: &MdnsAddressLookup, endpoint: &Endpoint) -> Result<()> {
    println!("trying to subscribe to mdns");
    let mut events = mdns.subscribe().await;
    println!("subscribed to mdns events");
    while let Some(event) = events.next().await  {
        match event {
            DiscoveryEvent::Discovered { endpoint_info, .. } => {
                println!("mdns discovered {:?}", endpoint_info);
                let conn = endpoint.connect(endpoint_info.endpoint_id, PIGEON_ALPN).await?;
                let (mut send, mut recv) = conn.open_bi().await.anyerr()?;
                send.write_all(&[1]).await.anyerr()?;
                exchange_usernames(&mut send, &mut recv, endpoint_info).await?;
                conn.close(VarInt::from_u32(0u32), b"");
            }
            DiscoveryEvent::Expired { endpoint_id } => {
                println!("mdns expired: {endpoint_id}");
            }
            _ => { println!("something weird happened") }
        }
    }
    Ok(())
}

//owned endpoint so it can be spawned from a separate thread
pub async fn exchange_info_mdns(endpoint: Endpoint) -> Result<()> {
    let mdns = MdnsAddressLookup::builder().build(endpoint.id())?;
    endpoint.address_lookup().unwrap().add(mdns.clone());

    MDNS_USERS.get_or_init(|| RwLock::new(HashMap::new()));

    subscribe_mdns_events(&mdns, &endpoint).await?;

    Ok(())
}

pub async fn get_endpoint_info_mdns(target: &ArrayString<32>) -> Option<EndpointInfo> {
    MDNS_USERS.get().unwrap().read().await.get(target).map(|k| k.clone())
}
