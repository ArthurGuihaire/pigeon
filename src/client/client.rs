use n0_error::Result;

mod common;
mod connect;
mod listen;

use common::{create_name_and_register, load_or_create_identity, try_load_name};
use listen::listen;
use connect::connect;
use pigeon::constants;

use crate::common::user_get_public_key;

#[tokio::main]
async fn main() -> Result<()> {
    let key = load_or_create_identity(&constants::DATA_DIR).map_err(|_| eprintln!("Error: cannot load key")).unwrap();
    let result = try_load_name(&constants::DATA_DIR);
    let username = match result {
        Ok(username) => username,
        Err(_) => {
            create_name_and_register(&constants::DATA_DIR, &key.public()).await.unwrap()
        }
    };
    common::USERNAME.set(username);

    tokio::spawn(listen());
    let target_key = user_get_public_key().await;
    connect(&target_key).await.unwrap();

    Ok(())
}
