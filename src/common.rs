use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use iroh::{Endpoint, PublicKey, SecretKey, Signature, endpoint::presets};
use iroh_mdns_address_lookup::MdnsAddressLookup;
use n0_error::{Result, StackResultExt, StdResultExt};
use arrayvec::{ArrayString, CapacityError};
use reqwest::StatusCode;
use crate::{GetKeyRequest, RegisterRequest, AuthRequest, ChangeNameRequest};
use crate::constants::{CHANGE_NAME_URL, DATA_DIR, GETKEY_URL, NAME_FILE, REGISTER_URL, START_AUTH_URL};
use rand::Rng;

pub const PIGEON_ALPN: &[u8] = b"pigeon/0";
pub static USERNAME: OnceLock<ArrayString<32>> = OnceLock::new();
pub static SECRET_KEY: OnceLock<SecretKey> = OnceLock::new();

pub fn try_load_name(name_path: &Path) -> Result<ArrayString<32>> {
    if name_path.exists() {
        let name_bytes = std::fs::read(name_path).std_context("read name file")?;
        let name_arraystring: ArrayString<32> = ArrayString::from(&String::from_utf8(name_bytes).expect("Name file is not UTF-8").trim()).expect("Cannot convert name to arraystring");
        Ok(name_arraystring)
    }
    else {
        Err("No name file".into())
    }
}

pub async fn read_name() -> ArrayString<32> {
    let mut name_string = String::new();
    print!("Choose a new name: ");
    loop {
        let _ = std::io::stdout().flush();
        std::io::stdin().read_line(&mut name_string).expect("IO error while reading from stdin");
        let result: Result<ArrayString<32>, CapacityError<&str>> = ArrayString::from(&name_string.trim());
        if let Ok(name_arraystring) = result {
            return name_arraystring;
        }
        else {
            println!("Something's wrong with that name, its probably too long");
            print!("Choose a different name:")
        }
    }
}

pub async fn create_name_and_register(path_prefix: &Path, publickey: &PublicKey) -> Result<ArrayString<32>> {
    let name_path = path_prefix.join(NAME_FILE);
    let name_arraystring = loop {
        let potential_name = read_name().await;
        let result = register_http(&potential_name, publickey).await;
        if result.is_ok() { break potential_name }
        else { println!("That name is taken"); }
    };

    if let Some(parent) = name_path.parent() && !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).std_context("create config dir")?;
    }
    std::fs::write(name_path, name_arraystring.to_string()).std_context("write name file")?;
    return Ok(name_arraystring);
}

pub fn load_or_create_identity(key_path: &Path) -> Result<SecretKey> {
    println!("trying to load {}", key_path.display());
    if key_path.exists() {
        let key = std::fs::read(key_path).std_context("read identity file")?;
        let key_bytes = hex::decode(key).expect("key file is not hex");
        Ok(SecretKey::from_bytes(&key_bytes.try_into().unwrap()))
    } else {
        if let Some(parent) = key_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).std_context("create config dir")?;
        }
        let key = SecretKey::generate();

        println!("Generated new key pair. public key signature: ");
        println!("{}", hex::encode(key.public()));

        std::fs::write(key_path, hex::encode(key.to_bytes())).std_context("write identity file")?;
        Ok(key)
    }
}

/// Binds an endpoint with preset N0
pub async fn bind_endpoint(secret_key: SecretKey) -> Result<Endpoint> {
    Endpoint::builder(presets::N0)
        .address_lookup(MdnsAddressLookup::builder())
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
                // for network errors, wait a bit and try again
                eprintln!("network error: {}", e);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

pub async fn get_public_key_interactive() -> PublicKey {
    let mut buf = String::new();
    loop {
        print!("Target username: ");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_line(&mut buf).expect("IO error while reading from stdin");
        let name: ArrayString<32> = ArrayString::from(buf.trim()).unwrap();
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

pub async fn verify_server_identity(auth_url: &str, expected_public_key: &PublicKey) -> bool {
    let mut random_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut random_bytes);
    let random_hex = hex::encode(&random_bytes);
    let client = reqwest::Client::new();
    let response: String = client.post(auth_url).json(&random_hex).send().await.expect("request failed").json().await.expect("cannot deserialize json");
    let signature_bytes = hex::decode(response).expect("response contains invalid hex data");
    let signature = Signature::from_bytes(&signature_bytes.try_into().unwrap());
    expected_public_key.verify(&random_bytes, &signature).is_ok()
}

pub async fn change_name(new_name: &ArrayString<32>, secret_key: &SecretKey) -> Result<()> {
    let old_name = USERNAME.get().expect("Getting name failed").clone();
    let auth_request = AuthRequest {
        name: old_name,
    };
    let client = reqwest::Client::new();
    let response = client.post(&*START_AUTH_URL).json(&auth_request).send().await.anyerr()?;
    //for some reason getting .text moves the response??? so get status beforehand
    let status = response.status();
    let text = response.text().await.anyerr()?;
    if status != StatusCode::OK {
        eprintln!("Received error response: {}", text);
        return Err("start_auth failed".into())
    }

    let challenge_bytes = hex::decode(&text).anyerr()?;
    let signature = secret_key.sign(&challenge_bytes);
    let change_name_request = ChangeNameRequest {
        old_name,
        new_name: *new_name,
        hex_signature: hex::encode(&signature.to_bytes()),
    };

    let response = client.post(&*CHANGE_NAME_URL).json(&change_name_request).send().await.anyerr()?;
    match response.status() {
        StatusCode::UNAUTHORIZED => return Err("Error: Unauthorized. must start auth first".into()),
        StatusCode::INTERNAL_SERVER_ERROR => return Err("Error: Internal server error".into()),
        StatusCode::FORBIDDEN => return Err("Error: Authentication failed".into()),
        StatusCode::EXPECTATION_FAILED => return Err("Error: Expectation failed".into()),
        StatusCode::OK => {}, // do nothing, let the function keep running
        status_code => return Err(format!("Error: unexpected status code {}, something went very wrong", status_code).into()),
    }

    std::fs::write(DATA_DIR.join(NAME_FILE), new_name.to_string())?;

    Ok(())
}

pub async fn change_name_interactive(secret_key: &SecretKey) -> Result<()> {
    let new_name = loop {
        let potential_name = read_name().await;
        let result = get_public_key(&potential_name).await; // use get_public_key cause we actually know why it fails when it does
        if let Err(err) = result && let Some(status) = err.status() && status == StatusCode::BAD_REQUEST {
            break potential_name;
        }
        println!("That name is already taken, or a different error occured");
    };

    change_name(&new_name, secret_key).await
}
