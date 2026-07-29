use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use iroh::{Endpoint, PublicKey, SecretKey, endpoint::presets};
use n0_error::{Result, StackResultExt, StdResultExt};
use arrayvec::ArrayString;
use pigeon::{GetKeyRequest, RegisterRequest};
use pigeon::constants::{GETKEY_URL, REGISTER_URL, NAME_FILE, KEY_FILE};

pub const PIGEON_ALPN: &[u8] = b"pigeon/0";

pub static USERNAME: OnceLock<ArrayString<32>> = OnceLock::new();

pub fn try_load_name(path_prefix: &Path) -> Result<ArrayString<32>> {
    let name_path = path_prefix.join(NAME_FILE);
    if name_path.exists() {
        let name_bytes = std::fs::read(name_path).std_context("read name file")?;
        let name_arraystring: ArrayString<32> = ArrayString::from(&String::from_utf8(name_bytes).expect("Name file is not UTF-8").trim()).expect("Cannot convert name to arraystring");
        Ok(name_arraystring)
    }
    else {
        Err("No name file".into())
    }
}

pub async fn create_name_and_register(path_prefix: &Path, publickey: &PublicKey) -> Result<ArrayString<32>> {
    let name_path = path_prefix.join(NAME_FILE);
    let mut name_string = String::new();
    let mut name_ararystring: ArrayString<32>;
    print!("No name set. choose your name: ");
    loop {
        let _ = std::io::stdout().flush();
        std::io::stdin().read_line(&mut name_string).expect("IO error while reading from stdin");
        name_ararystring = ArrayString::from(&name_string.trim()).expect("bad stdin input");

        let result = register_http(&name_ararystring, publickey).await;
        if let Ok(_) = result {
            break;
        }
        print!("choose a different name: ")
    }

    if let Some(parent) = name_path.parent() && !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).std_context("create config dir")?;
    }
    std::fs::write(name_path, name_string.trim()).std_context("write name file")?;
    return Ok(name_ararystring);
}

pub fn load_or_create_identity(path_prefix: &Path) -> Result<SecretKey> {
    let key_path = path_prefix.join(KEY_FILE);
    if key_path.exists() {
        let key = std::fs::read(key_path).std_context("read identity file")?;
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&key);
        Ok(SecretKey::from_bytes(&key_bytes))
    } else {
        if let Some(parent) = key_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).std_context("create config dir")?;
        }
        let key = SecretKey::generate();

        std::fs::write(key_path, key.to_bytes()).std_context("write identity file")?;
        Ok(key)
    }
}

/// Binds an endpoint with preset N0
pub async fn bind_endpoint(secret_key: SecretKey) -> Result<Endpoint> {
    Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .alpns(vec![PIGEON_ALPN.to_vec()])
        .bind()
        .await
        .context("bind endpoint")
}

pub async fn get_public_key(
    target: &ArrayString<32>,
) -> Result<PublicKey, reqwest::Error> {
    let client = reqwest::Client::new();

    let request = GetKeyRequest { target: *target };

    loop {
        //send post
        let response = client.get(&(*GETKEY_URL)).json(&request).send().await;

        match response {
            Ok(res) => {
                let status = res.status();
                if let Err(reqwest_err) = res.error_for_status_ref() {
                    let error_text = res.text().await?;
                    println!("error response: {}: {}", status, error_text);

                    return Err(reqwest_err);
                }
                if status.is_success() {
                    let publickey: PublicKey = res.json().await.unwrap();
                    return Ok(publickey)
                }
                // other responses are unexpected, something went wrong
            }
            Err(e) => {
                // for network errors, try again
                eprintln!("network error: {}", e);
            }
        }
    }
}

pub async fn user_get_public_key() -> PublicKey {
    let mut buf = String::new();
    loop {
        print!("Target username: ");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_line(&mut buf).expect("IO error while reading from stdin");
        let name: ArrayString<32> = ArrayString::from(&buf).unwrap();
        let result = get_public_key(&name).await;
        match result {
            Ok(key) => return key,
            Err(e) => { eprintln!("Error getting public key for target: {e}") }
        }
    }
}

pub async fn register_http(name: &ArrayString<32>, publickey: &PublicKey) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();

    let request = RegisterRequest {
        name: *name,
        publickey: *publickey,
    };

    loop {
        let response = client.post(&(*REGISTER_URL)).json(&request).send().await;

        match response {
            Ok(res) => {
                let status = res.status();
                if let Err(reqwest_err) = res.error_for_status_ref() {
                    let error_text = res.text().await?;
                    println!("error response: {}: {}", status, error_text);

                    return Err(reqwest_err);
                }
                if status.is_success() {
                    return Ok(())
                }
                // other responses are unexpected, something went wrong
            }
            Err(e) => {
                // for network errors, try again
                eprintln!("network error: {}", e);
            }
        }
    }
}
