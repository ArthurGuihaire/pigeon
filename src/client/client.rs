use std::path::PathBuf;

use n0_error::{Result, StdResultExt};

mod common;
mod connect;
mod listen;

use common::{create_name_and_register, load_or_create_identity, try_load_name};
use listen::listen;
use connect::connect_and_send;
use pigeon::constants;

use crate::common::{get_public_key, register_http, user_get_public_key};

#[tokio::main]
async fn main() -> Result<()> {
    let key = load_or_create_identity(&constants::DATA_DIR).expect("Error: cannot load key");
    let result = try_load_name(&constants::DATA_DIR);
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
    common::USERNAME.set(username).expect("USERNAME already set");

    let join_handle = tokio::spawn(listen());

    let send_path_option = std::env::args().nth(1);
    if let Some(send_path_string) = send_path_option {
        let send_path = PathBuf::from(send_path_string);
        let target_key = user_get_public_key().await;
        connect_and_send(&target_key, &send_path).await.unwrap();
    }
    else {
        join_handle.await.expect("something went wrong").expect("something went wrong");
    }

    Ok(())
}
