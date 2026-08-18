use arrayvec::ArrayString;
use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use axum_server::tls_rustls::RustlsConfig;
use iroh::PublicKey;
use iroh::Signature;
use pigeon::AuthRequest;
use pigeon::ChangeNameRequest;
use pigeon::GetKeyRequest;
use rand::Rng;
use std::sync::Arc;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::Mutex;

use pigeon::RegisterRequest;

#[derive(Clone)]
struct SharedState {
    clients: Arc<Mutex<HashMap<ArrayString<32>, (PublicKey, Option<[u8; 32]>)>>>,
}
//let config = RustlsConfig::from_pem_file("cert.pem", "key.pem").await.expect("failed to load tls keys");

#[tokio::main]
async fn main() {
    let state = SharedState {
        clients: Arc::new(Mutex::new(HashMap::new())),
    };
    let app: Router = Router::new()
        .route("/register", post(handle_registration))
        .route("/getkey", get(handle_key_request))
        .route("/start_auth", post(start_auth))
        .route("/change_name", post(change_name))
        .with_state(state);

    rustls::crypto::ring::default_provider().install_default().expect("failed to set default rustls crypto provider");
    let config = RustlsConfig::from_pem_file("cert.pem", "key.pem")
        .await
        .expect("Failed to load cert.pem or key.pem");
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));

    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await.unwrap();
    // let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    // axum::serve(listener, app).await.unwrap();
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
        db.insert(payload.name, (payload.publickey, None));
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
        Some(key) => Ok(Json(key.0)),
    }
}

async fn start_auth(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::Json(payload): axum::extract::Json<AuthRequest>) -> (StatusCode, String)
{
    let mut db = state.clients.lock().await;
    let result = db.get_mut(&payload.name);
    let entry = match result {
        None => return (StatusCode::BAD_REQUEST, String::from("cannot change a name that is not registered")),
        Some(val) => val,
    };
    let mut challenge_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut challenge_bytes);
    entry.1 = Some(challenge_bytes);
    let challenge = hex::encode(&challenge_bytes);
    println!("{}", challenge);

    (StatusCode::OK, challenge)
}

async fn change_name(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::Json(payload): axum::extract::Json<ChangeNameRequest>) -> StatusCode
{
    let mut db = state.clients.lock().await;
    let result = db.get_mut(&payload.old_name);
    let entry = match result {
        None => return StatusCode::UNAUTHORIZED,
        Some(val) => val,
    };
    let signature_bytes_result = hex::decode(payload.hex_signature);
    let signature_bytes = match signature_bytes_result {
        Err(e) => {
            eprintln!("Server error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        },
        Ok(val) => val,
    };
    let result = entry.0.verify(&entry.1.unwrap(), &Signature::from_bytes(&signature_bytes.try_into().unwrap()));
    if result.is_err() {
        return StatusCode::FORBIDDEN
    }
    let collision = db.contains_key(&payload.new_name);
    if collision {
        return StatusCode::BAD_REQUEST
    }
    let old_entry = db.remove(&payload.old_name);
    match old_entry {
        None => StatusCode::EXPECTATION_FAILED,
        Some(entry) => {
            db.insert(payload.new_name, entry);
            db.get_mut(&payload.new_name).unwrap().1 = None; //reset auth
            StatusCode::OK
        }
    }
}
