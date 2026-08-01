use arrayvec::ArrayString;
use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use ed25519_dalek::Signer;
use iroh::PublicKey;
use pigeon::GetKeyRequest;
use pigeon::constants::SERVER_KEY_FILE_1;
use pigeon::constants::SERVER_KEY_FILE_2;
use std::path::Path;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use ed25519_dalek::{SigningKey};

use pigeon::RegisterRequest;

use crate::utils::generate_and_save_private_key;
use crate::utils::load_private_key;

#[derive(Clone)]
struct SharedState {
    clients: Arc<Mutex<HashMap<ArrayString<32>, PublicKey>>>,
    private_key: SigningKey,
}

mod utils;

#[tokio::main]
async fn main() {
    let key_path_1 = Path::new(SERVER_KEY_FILE_1);
    let key_path_2 = Path::new(SERVER_KEY_FILE_2);
    let result = load_private_key(&key_path_1);
    let private_key = match result {
        Ok(key) => key,
        Err(_) => match load_private_key(&key_path_2) {
            Ok(key) => key,
            Err(_) => generate_and_save_private_key(&key_path_1).expect("failed to generate private key"),
        }
    };
    let state = SharedState {
        clients: Arc::new(Mutex::new(HashMap::new())),
        private_key,
    };
    let app: Router = Router::new()
        .route("/register", post(handle_registration))
        .route("/getkey", get(handle_key_request))
        .route("/auth", post(handle_auth))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_registration(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::Json(payload): axum::extract::Json<RegisterRequest>,
) -> Result<StatusCode, (StatusCode, &'static str)>
{
    let mut db = state.clients.lock().await;
    if db.contains_key(&payload.name) {
        Err((StatusCode::BAD_REQUEST, "that name is already registered"))
    }
    else {
        db.insert(payload.name, payload.publickey);
        Ok(StatusCode::CREATED)
    }
}

async fn handle_key_request(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::Json(payload): axum::extract::Json<GetKeyRequest>,
) -> Result<Json<PublicKey>, (StatusCode, String)>
{
    let db = state.clients.lock().await;
    match db.get(&payload.target) {
        None => Err((StatusCode::BAD_REQUEST, format!("{} is not registered", &payload.target))),
        Some(key) => Ok(Json(*key)),
    }
}

async fn handle_auth(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::Json(payload): axum::extract::Json<String>) -> Json<String>
{
    let bytes = hex::decode(payload).expect("not hex");
    let signature = state.private_key.sign(&bytes);

    Json(hex::encode(signature.to_bytes()))
}
