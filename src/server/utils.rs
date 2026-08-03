use ed25519_dalek::SigningKey;
use rand::rng;
use rand::rngs::ThreadRng;
use std::fs;
use std::path::Path;
use n0_error::Result;

pub fn generate_and_save_private_key(path: &Path) -> Result<SigningKey> {
    //generate a key
    let mut rng: ThreadRng = rng();
    let signing_key = SigningKey::generate(&mut rng);

    //save the key to file
    let signing_key_hex = hex::encode(signing_key.to_bytes());
    fs::write(path, &signing_key_hex)?;

    println!("Created new key pair. Public key signature:");
    println!("{}", hex::encode(signing_key.verifying_key().as_bytes()));

    Ok(signing_key)
}

pub fn load_private_key(path: &Path) -> Result<SigningKey> {
    let hex_bytes = fs::read(path)?;
    let mut bytes = hex::decode(&hex_bytes).expect("file does not contain hex");

    while bytes.last() == Some(&b'\n') || bytes.last() == Some(&b'\r') || bytes.last() == Some(&b' ') {
        bytes.pop();
    }

    Ok(SigningKey::from_bytes(&bytes.try_into().map_err(|b: Vec<u8>| format!("Invalid key length: expected 32 bytes, got {}", b.len()))?))

}
