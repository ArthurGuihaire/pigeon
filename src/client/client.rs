use std::path::PathBuf;

use n0_error::Result;

mod common;
mod connect;
mod listen;

use common::{create_name_and_register, load_or_create_identity, try_load_name};
use listen::listen;
use connect::connect_and_send;
use pigeon::constants;

use crate::common::user_get_public_key;

#[tokio::main]
async fn main() -> Result<()> {
    let send_path_string = std::env::args().nth(1).expect("no pattern given");
    let send_path = PathBuf::from(send_path_string);
    let key = load_or_create_identity(&constants::DATA_DIR).expect("Error: cannot load key");
    let result = try_load_name(&constants::DATA_DIR);
    let username = match result {
        Ok(username) => username,
        Err(_) => {
            create_name_and_register(&constants::DATA_DIR, &key.public()).await.unwrap()
        }
    };
    common::USERNAME.set(username).expect("USERNAME already set");

    tokio::spawn(listen());
    let target_key = user_get_public_key().await;
    connect_and_send(&target_key, &send_path).await.unwrap();

    Ok(())
}
