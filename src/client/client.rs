use std::path::PathBuf;
use iroh::PublicKey;
use n0_error::{Result, StdResultExt};

mod connect;
mod listen;

use pigeon::common::{SECRET_KEY, change_name_interactive, create_name_and_register, load_or_create_identity, try_load_name};
use listen::listen;
use connect::connect_and_send;
use pigeon::constants::{self, AUTH_URL, SERVER_PUBLIC_KEY};

use pigeon::common::{get_public_key, register_http, get_public_key_interactive, verify_server_identity};

#[tokio::main]
async fn main() -> Result<()> {
    let server_authenticity = verify_server_identity(&AUTH_URL, &PublicKey::from_bytes(&hex::decode(SERVER_PUBLIC_KEY).unwrap().try_into().unwrap()).unwrap()).await;
    if !server_authenticity {
        panic!("Server authenticity cannot be established");
    }
    let key = load_or_create_identity(&constants::DATA_DIR.join(constants::CLIENT_KEY_FILE)).expect("Error: cannot load key");
    SECRET_KEY.set(key.clone()).expect("SECRET_KEY already set");
    let result = try_load_name(&constants::DATA_DIR.join(constants::NAME_FILE));
    let username = match result {
        Ok(username) => {
            let result = get_public_key(&username).await;
            match result {
                Err(_) => {
                    register_http(&username, &key.public()).await.anyerr()?;
                    username
                }
                Ok(server_key) => {
                    if server_key == key.public() {
                        username
                    }
                    else {
                        println!("Server and local public keys do not match, create a new identity");
                        create_name_and_register(&constants::DATA_DIR, &key.public()).await.anyerr()?
                    }
                }
            }
        },
        Err(_) => {
            create_name_and_register(&constants::DATA_DIR, &key.public()).await.unwrap()
        }
    };
    pigeon::common::USERNAME.set(username).expect("USERNAME already set");

    let join_handle = tokio::spawn(listen());

    let arg_string_option = std::env::args().nth(1);
    if let Some(arg_string) = arg_string_option {
        if arg_string.starts_with("--") {
            let option_string = arg_string.get(2..).unwrap();
            match option_string {
                "change-name" => {
                    change_name_interactive(SECRET_KEY.get().expect("failed to get private key")).await.expect("failed to change name");
                },
                _ => {
                    eprintln!("option not recognized, exiting");
                }
            }
        }
        else {
            let send_path = PathBuf::from(arg_string);
            let target_key = get_public_key_interactive().await;
            connect_and_send(&target_key, &send_path).await.unwrap();
        }
    }
    else {
        join_handle.await.expect("something went wrong").expect("something went wrong");
    }

    Ok(())
}
