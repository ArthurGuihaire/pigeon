use std::path::PathBuf;
use std::sync::OnceLock;
use iroh::PublicKey;
use n0_error::{Result, StdResultExt};

mod connect;
mod listen;
mod mdns;
mod utils;

use pigeon::common::{SECRET_KEY, MDNS_USERNAME, ONLINE_USERNAME, change_name_interactive, create_name_and_register, load_or_create_identity, try_load_name};
use listen::listen;
use pigeon::constants::{self, AUTH_URL, SERVER_PUBLIC_KEY};

use pigeon::common::{get_public_key, register_http, verify_server_identity};

use crate::utils::create_endpoint;
use crate::mdns::exchange_info_mdns;
use crate::utils::{DiscoveryType, get_endpoint_info_interactive};
use crate::connect::connect_and_send;

pub static USE_SERVER: OnceLock<bool> = OnceLock::new();

async fn online_thread() -> Result<()> {
    let server_authenticity = verify_server_identity(&AUTH_URL, &PublicKey::from_bytes(&hex::decode(SERVER_PUBLIC_KEY).anyerr()?.try_into().unwrap())?).await.inspect_err(|_| { let _ = USE_SERVER.set(false); } )?;
    if !server_authenticity { let _ = USE_SERVER.set(false); return Err("Server authenticity cannot be established, using only mdns".into()); }

    USE_SERVER.set(true).expect("Can't set USE_SERVER for some reason");

    let current_username = MDNS_USERNAME.get().unwrap();
    let key = SECRET_KEY.get().unwrap();
    let result = get_public_key(&current_username).await;
    match result {
        Err(_) => {
            register_http(&current_username, &key.public()).await.anyerr()?;
        }
        Ok(server_key) => {
            if server_key == key.public() {
                ONLINE_USERNAME.set(current_username.clone()).unwrap();
            } else {
                println!("Server and local public keys do not match, create a new identity");
                ONLINE_USERNAME.set(create_name_and_register(&constants::DATA_DIR, &key.public(), true).await.anyerr()?).unwrap();
            }
        }
    }

    println!("online thread finished");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let key = load_or_create_identity(&constants::DATA_DIR.join(constants::CLIENT_KEY_FILE)).expect("Error: cannot load key");
    SECRET_KEY.set(key.clone()).expect("SECRET_KEY already set");
    let result = try_load_name(&constants::DATA_DIR.join(constants::NAME_FILE));
    let username = match result {
        Ok(username) => {
            username
        },
        Err(_) => {
            create_name_and_register(&constants::DATA_DIR, &key.public(), false).await?
        }
    };
    pigeon::common::MDNS_USERNAME.set(username).expect("USERNAME already set");

    let online_thread = tokio::spawn(online_thread());

    let endpoint = create_endpoint().await?;

    let mdns_lookup_thread = tokio::spawn(exchange_info_mdns(endpoint.clone()));
    let listen_thread = tokio::spawn(listen(endpoint.clone()));

    let arg_string_option = std::env::args().nth(1);
    if let Some(arg_string) = arg_string_option {
        if arg_string.starts_with("--") {
            let option_string = arg_string.get(2..).unwrap();
            match option_string {
                "change-name" => {
                    change_name_interactive(SECRET_KEY.get().unwrap()).await?;
                },
                _ => {
                    eprintln!("option not recognized, exiting");
                }
            }
        }
        else {
            let send_path = PathBuf::from(arg_string);
            // let target_key = if is_online { get_public_key_interactive().await } else { get_public_key_mdns_interactive().await };
            let (target_info, discovery_type) = get_endpoint_info_interactive().await;
            let connect_name = match discovery_type {
                DiscoveryType::MDNS => MDNS_USERNAME.get().unwrap(),
                DiscoveryType::SERVER => ONLINE_USERNAME.get().unwrap(),
            };
            connect_and_send(&endpoint, &target_info, &send_path, connect_name).await?;

            if !listen_thread.is_finished() { listen_thread.abort(); }
        }
    }
    else {
        listen_thread.await.anyerr()??;
    }

    if !mdns_lookup_thread.is_finished() {
        mdns_lookup_thread.abort();
        let _ = mdns_lookup_thread.await;
    }
    if !online_thread.is_finished() {
        online_thread.abort();
        let _ = online_thread.await;
    }

    endpoint.close().await;

    println!("everything is done running, should exit now");

    return Ok(())
}
