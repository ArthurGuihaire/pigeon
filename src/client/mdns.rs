use std::{collections::HashMap, io::Write, str::{self, FromStr}, sync::OnceLock};
use arrayvec::ArrayString;
use iroh::{Endpoint, EndpointId, PublicKey, endpoint::{RecvStream, SendStream}};
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
use n0_error::{AnyError, Result, StdResultExt};
use n0_future::StreamExt;
use pigeon::common::{PIGEON_ALPN, MDNS_USERNAME};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, sync::RwLock};

pub static MDNS_USERS: OnceLock<RwLock<HashMap<ArrayString<32>, EndpointId>>> = OnceLock::new();

pub async fn exchange_usernames(send: &mut SendStream, recv: &mut RecvStream, target: EndpointId) -> Result<()> {
    let read_future = async {
        println!("read waiting");
        let size = recv.read_u8().await? as usize;
        println!("read size {size}");
        let mut buf = [0u8; 32];
        recv.read_exact(&mut buf[..size]).await.anyerr()?;

        let name = ArrayString::from_str(str::from_utf8(&buf[..size]).anyerr()?).anyerr()?;
        println!("Mdns found user with name {name}");
        MDNS_USERS.get().unwrap().write().await.insert(name, target);

        Ok::<(), AnyError>(())
    };

    let write_future = async {
        let serialized = MDNS_USERNAME.get().unwrap().as_bytes();
        println!("writing size {}", serialized.len());
        send.write_u8(serialized.len() as u8).await?;
        println!("writing buffer");
        send.write_all(&serialized).await.anyerr()?;
        send.finish().anyerr()?;
        send.flush().await?;
        println!("finished writing");
        Ok::<(), AnyError>(())
    };

    let (read_result, write_result) = tokio::join!(read_future, write_future);
    read_result?;
    write_result?;

    return Ok(())
}

async fn subscribe_mdns_events(mdns: &MdnsAddressLookup, endpoint: &Endpoint) -> Result<()> {
    let mut events = mdns.subscribe().await;
    while let Some(event) = events.next().await  {
        match event {
            DiscoveryEvent::Discovered { endpoint_info, .. } => {
                println!("mdns discovered {:?}", endpoint_info);
                let conn = endpoint.connect(endpoint_info.endpoint_id, PIGEON_ALPN).await?;
                let (mut send, mut recv) = conn.open_bi().await.anyerr()?;
                send.write_all(&[1]).await.anyerr()?;
                exchange_usernames(&mut send, &mut recv, endpoint_info.endpoint_id).await?;
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

pub async fn get_public_key_mdns(target: &ArrayString<32>) -> Option<PublicKey> {
    MDNS_USERS.get().unwrap().read().await.get(target).map(|k| k.clone())
}

#[deprecated]
pub async fn get_public_key_mdns_interactive() -> PublicKey {
    let mut buf = String::new();
    loop {
        print!("Target username: ");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_line(&mut buf).expect("IO error while reading from stdin");
        let name: ArrayString<32> = ArrayString::from(buf.trim()).unwrap();
        let mdns_users_ref = MDNS_USERS.get().unwrap().read().await;
        let option = mdns_users_ref.get(&name);
        match option {
            Some(key) => return key.clone(),
            None => {
                println!("That peer has not been found on the local network. These have been found so far (without the quotes): ");
                let mut keys = mdns_users_ref.keys();
                if let Some(first) = keys.next() {
                    print!("\"{first}\"");
                    for peer in keys {
                        print!(", \"{peer}\"");
                    }
                }
            }
        }
    }
}
