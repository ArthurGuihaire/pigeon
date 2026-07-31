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
    fs::write(path, signing_key.to_bytes())?;

    println!("Created new key pair. Public key signature:");
    println!("{}", hex::encode(signing_key.verifying_key().as_bytes()));

    Ok(signing_key)
}

pub fn load_private_key(path: &Path) -> Result<SigningKey> {
    let bytes = fs::read(path)?;

    Ok(SigningKey::from_bytes(&bytes.try_into().map_err(|_| "invalid key length")?))

}
