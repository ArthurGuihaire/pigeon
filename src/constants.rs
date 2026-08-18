use std::{path::PathBuf, sync::LazyLock}; // Standard library in modern Rust
use directories::ProjectDirs;

static SERVER_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SERVER_URL").unwrap_or(String::from("https://pigeon-87r8.onrender.com"))
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

pub static START_AUTH_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/start_auth", *SERVER_URL)
});

pub static CHANGE_NAME_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/change_name", *SERVER_URL)
});

pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("", "", "pigeon")
        .map(|proj_dirs| proj_dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.local/share/pigeon"))
});
pub const NAME_FILE: &str = "username.txt";
pub const CLIENT_KEY_FILE: &str = "ed25519_key";

pub const CHUNK_SIZE: usize = 64 * 1024;

pub const USE_CUSTOM_HTTPS: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("USE_CUSTOM_HTTPS").map(|s| s.parse::<bool>().expect(&format!("USE_CUSTOM_HTTPS should only be true or false, not {}", s))).unwrap_or(false)
});
