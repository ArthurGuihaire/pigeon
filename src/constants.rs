use std::{path::PathBuf, sync::LazyLock}; // Standard library in modern Rust
use directories::ProjectDirs;

static SERVER_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SERVER_URL").map_err(|err| { eprintln!("SERVER_URL not set, failing"); err }).unwrap()
});
// Automatically reads NGROK_SERVER from .env or shell environment at runtime
pub static REGISTER_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/register", *SERVER_URL)
});

pub static GETKEY_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/getkey", *SERVER_URL)
});

pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("", "", "pigeon")
        .map(|proj_dirs| proj_dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.local/share/pigeon"))
});
pub const NAME_FILE: &str = "username.txt";
pub const KEY_FILE: &str = "ed25519_key";

pub const CHUNK_SIZE: usize = 1024 * 64; // 64 KiB
