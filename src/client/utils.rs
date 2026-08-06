use iroh::PublicKey;
use arrayvec::ArrayString;
use std::io::Write;
use crate::{USE_SERVER, get_public_key};
use crate::mdns::get_public_key_mdns;

pub enum DiscoveryType {
    MDNS,
    SERVER,
}

pub async fn get_public_key_interactive() -> (PublicKey, DiscoveryType) {
    let mut buf = String::new();
    loop {
        print!("Target username: ");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_line(&mut buf).expect("IO error while reading from stdin");
        let name: ArrayString<32> = ArrayString::from(buf.trim()).unwrap();
        let key_option_mdns = get_public_key_mdns(&name).await;
        if let Some(key) = key_option_mdns { return (key, DiscoveryType::MDNS) }
        //if mdns doesn't find it yet, then use the server
        if USE_SERVER.get().unwrap_or(&false).clone() {
            let result = get_public_key(&name).await;
            match result {
                Ok(key) => return (key, DiscoveryType::SERVER),
                Err(e) => {
                    eprintln!("Error getting public key for target: {e}");
                }
            }
        }
    }
}
