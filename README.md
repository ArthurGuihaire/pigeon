# Pigeon
> File sharing app that aims to be similar to localsend, but cross-network

## Installation
- On Windows, macOS, and Linux: install rust toolchain, run `cargo build --release`, and place the executable target/release/pigeon(.exe) in the desired location
- prebuild binaries / cargo-dist scripts coming soon
- Arch Linux: AUR package coming soonish (before August 15th hopefully)
- crates.io: might be published by August 15th as well
- Android support: maybe one day, ios probably never cause pain

## Usage
Listen/receive files: `pigeon`

Send files/folders: `pigeon /path/to/file/or/folder`

Change name: `pigeon --change-name`
> Running pigeon in any way for the first time will automatically generate encryption keys and prompt you to select a username
