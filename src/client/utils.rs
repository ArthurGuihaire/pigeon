use iroh::{Endpoint, PublicKey, SecretKey};
use arrayvec::{ArrayString, CapacityError};
use iroh::endpoint::{Connection, ConnectionError};
use iroh::endpoint_info::{EndpointData, EndpointInfo};
use n0_error::{anyerr, Result};
use n0_error::StdResultExt;
use pigeon::{AuthRequest, ChangeNameRequest, GetKeyRequest, RegisterRequest};
use pigeon::common::{ONLINE_USERNAME, SECRET_KEY, bind_endpoint};
use pigeon::constants::{CHANGE_NAME_URL, DATA_DIR, GETKEY_URL, NAME_FILE, REGISTER_URL, START_AUTH_URL};
use reqwest::{Certificate, Client, StatusCode};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, atomic};
use std::time::Duration;
use std::io::Write;
use crate::{USE_SERVER};
use crate::mdns::get_endpoint_info_mdns;

pub static PRINT_QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub static PRINT_BLOCKED: atomic::AtomicBool = AtomicBool::new(false);

pub fn safe_input(msg: &str, buf: &mut String) {
    PRINT_BLOCKED.store(true, atomic::Ordering::Relaxed);
    print!("{msg}");
    let _ = std::io::stdout().flush();
    let _ = std::io::stdin().read_line(buf);
    PRINT_BLOCKED.store(false, atomic::Ordering::Relaxed);
    flush_print_queue();
}

pub fn safe_print(msg: &str) {
    if !(PRINT_BLOCKED.load(atomic::Ordering::Relaxed)) {
        println!("{msg}");
    }
    else {
        PRINT_QUEUE.lock().unwrap().push(msg.to_string());
    }
}

pub fn flush_print_queue() {
    let mut queue = PRINT_QUEUE.lock().unwrap();
    for msg in queue.iter() {
        println!("{}", msg);
    }
    queue.clear();
}

// #[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_print_above {
    ($($arg:tt)*) => {
        crate::utils::safe_print(&format!($($arg)*))
    };
}

// #[cfg(not(debug_assertions))]
// #[macro_export]
// macro_rules! debug_print_above {
//     ($($arg:tt)*) => {};
// }

pub enum DiscoveryType {
    MDNS,
    SERVER,
}

pub async fn get_endpoint_info_interactive() -> (EndpointInfo, DiscoveryType) {
    let mut buf = String::new();
    loop {
        safe_input("Target username: ", &mut buf);
        let name: ArrayString<32> = ArrayString::from(buf.trim()).unwrap();
        let info_option_mdns = get_endpoint_info_mdns(&name).await;
        if let Some(info) = info_option_mdns { return (info, DiscoveryType::MDNS) }
        //if mdns doesn't find it yet, then use the server
        if USE_SERVER.get().unwrap_or(&false).clone() {
            let result = get_public_key(&name).await;
            match result {
                Ok(key) => return (EndpointInfo::from_parts(key, EndpointData::default()), DiscoveryType::SERVER),
                Err(e) => {
                    eprintln!("Error getting public key for target: {e}");
                }
            }
        }
        buf.clear();
    }
}

pub async fn get_endpoint_info(connection: &Connection, endpoint: &Endpoint) -> Result<EndpointInfo> {
    let remote_id = connection.remote_id();
    if let Some(info) = endpoint.remote_info(remote_id).await {
        Ok(EndpointInfo::from_parts(remote_id, EndpointData::new(info.into_addrs().map(|a| a.addr().clone()).collect())))
    }
    else {
        Err(anyerr!("Cannot get remote endpoint info"))
    }
}

pub async fn create_endpoint() -> Result<Endpoint> {
    let secret_key = SECRET_KEY.get().expect("Failed to load secret key");
    let endpoint = bind_endpoint(secret_key.clone()).await?;

    Ok(endpoint)
}

pub async fn safe_wait_connection_closed(connection: &Connection) {
    let remote_id = connection.remote_id();
    let res = tokio::time::timeout(Duration::from_secs(3), async move {
        let closed = connection.closed().await;
        if !matches!(closed, ConnectionError::ApplicationClosed(_)) {
            debug_print_above!("{remote_id} disconnected with an error: {closed:#}");
        }
    })
    .await;
    if res.is_err() {
        println!("{remote_id} did not disconnect within 3 seconds");
    }
}


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

    loop {
        safe_input("Choose a new name: ", &mut name_string);
        let result: Result<ArrayString<32>, CapacityError<&str>> = ArrayString::from(&name_string.trim());
        if let Ok(name_arraystring) = result {
            return name_arraystring;
        }
        else {
            println!("Something's wrong with that name, its probably too long");
        }
    }
}

pub async fn create_name_and_register(path_prefix: &Path, publickey: &PublicKey, is_online: bool) -> Result<ArrayString<32>> {
    let name_path = path_prefix.join(NAME_FILE);
    let name_arraystring = loop {
        let potential_name = read_name().await;
        if is_online {
            let result = register_http(&potential_name, publickey).await;
            if result.is_ok() { break potential_name }
            else { println!("That name is taken"); }
        }
        else { break potential_name }
    };

    if let Some(parent) = name_path.parent() && !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).std_context("create config dir")?;
    }
    std::fs::write(name_path, name_arraystring.to_string()).std_context("write name file")?;
    return Ok(name_arraystring);
}

const CERT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/rootCA.pem"
));

fn build_client() -> Result<Client> {
    let ca_cert = Certificate::from_pem(&CERT_BYTES).anyerr()?;
    reqwest::Client::builder().tls_certs_merge([ca_cert]).build().anyerr()
}

pub async fn change_name(new_name: &ArrayString<32>, secret_key: &SecretKey) -> Result<()> {
    let old_name = ONLINE_USERNAME.get().expect("Cannot change online username unless you are connected to the internet").clone();
    let auth_request = AuthRequest {
        name: old_name,
    };
    let client = build_client()?;
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

    safe_print(&format!("Successfully changed name to {}", new_name));

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

pub async fn get_public_key(
    target: &ArrayString<32>,
) -> Result<PublicKey, reqwest::Error> {
    let client = build_client().expect("failed to create http client");

    let request = GetKeyRequest { target: *target };

    loop {
        //send post
        let response = client.get(&(*GETKEY_URL)).json(&request).send().await;

        match response {
            Ok(res) => {
                let status = res.status();
                if let Err(reqwest_err) = res.error_for_status_ref() {
                    let error_text = res.text().await?;
                    debug_print_above!("error response: {}: {}", status, error_text);

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
                safe_print(&format!("network error: {}", e));
                if let Some(source) = std::error::Error::source(&e) {
                    debug_print_above!("Caused by: {:?}", source);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

pub async fn register_http(name: &ArrayString<32>, publickey: &PublicKey) -> Result<(), reqwest::Error> {
    let client = build_client().expect("failed to build http client");

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
                    debug_print_above!("error response: {}: {}", status, error_text);

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
