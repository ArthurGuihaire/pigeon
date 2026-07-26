use arrayvec::ArrayString;
use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use iroh::PublicKey;
use pigeon::GetKeyRequest;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;

use pigeon::RegisterRequest;

type SharedState = Arc<Mutex<HashMap<ArrayString<32>, PublicKey>>>;

#[tokio::main]
async fn main() {
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
    let app: Router = Router::new()
        .route("/register", post(handle_registration))
        .route("/getkey", get(handle_key_request))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_registration(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::Json(payload): axum::extract::Json<RegisterRequest>,
) -> Result<StatusCode, (StatusCode, &'static str)>
{
    let mut db = state.lock().await;
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
) -> Result<Json<PublicKey>, (StatusCode, &'static str)>
{
    let db = state.lock().await;
    match db.get(&payload.target) {
        None => Err((StatusCode::BAD_GATEWAY, "that name is not registered")),
        Some(key) => Ok(Json(*key)),
    }
}
