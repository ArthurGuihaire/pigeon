use std::{path::PathBuf, sync::LazyLock}; // Standard library in modern Rust
use directories::ProjectDirs;

static SERVER_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SERVER_URL").expect("SERVER_URL not set, failing")
});
pub static REGISTER_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/register", *SERVER_URL)
});

pub static GETKEY_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/getkey", *SERVER_URL)
});

pub static AUTH_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/auth", *SERVER_URL)
});

pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("", "", "pigeon")
        .map(|proj_dirs| proj_dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.local/share/pigeon"))
});
pub const NAME_FILE: &str = "username.txt";
pub const CLIENT_KEY_FILE: &str = "ed25519_key";
pub const SERVER_KEY_FILE: &str = "ed25519_signing_key";

pub const SERVER_PUBLIC_KEY: &str = "616b8ece0c39df9fa236484181f620ba410f8c7cd15f2862d137e1a35de15ce8";

pub const CHUNK_SIZE: usize = 1024 * 64; // 64 KiB
