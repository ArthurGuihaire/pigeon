pub mod constants;
use iroh::PublicKey;
use serde::{Serialize, Deserialize};
use arrayvec::ArrayString;
#[derive(Deserialize, Serialize)]
pub struct RegisterRequest {
    pub name: ArrayString<32>,
    pub publickey: PublicKey,
}

#[derive(Serialize, Deserialize)]
pub struct GetKeyRequest {
    pub target: ArrayString<32>,
}
