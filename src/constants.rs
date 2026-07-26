use std::{path::PathBuf, sync::LazyLock}; // Standard library in modern Rust
use directories::ProjectDirs;

static NGROK_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("NGROK_SERVER").map_err(|err| { eprintln!("NGROK_SERVER not set, failing"); err }).unwrap()
});
// Automatically reads NGROK_SERVER from .env or shell environment at runtime
pub static REGISTER_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/register", *NGROK_URL)
});

pub static GETKEY_URL: LazyLock<String> = LazyLock::new(|| {
    format!("{}/getkey", *NGROK_URL)
});

pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("", "", "pigeon")
        .map(|proj_dirs| proj_dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.local/share/pigeon"))
});
pub const NAME_FILE: &str = "username.txt";
pub const KEY_FILE: &str = "ed25519_key.pub";
