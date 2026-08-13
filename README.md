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

Re-generating encryption keys is not natively supported, but you can manually delete the keys inside the app's data directory. On windows its `%appdata%/pigeon`, on macos its `/Library/Application Support/pigeon`, on linux its `~/.local/share/pigeon`. You can edit the username file inside the data folder as well, but this does not update the server side registry, so using --change-name is the preferred method.

## Data privacy and encryption
The names and types of files you send and receive and their contents are end-to-end encrypted and the server never sees any verison of them. Connections with the server are authenticated, but not encrypted since they go through http (this may change in the future). Only your username and public key is ever shared with the server. The server is only used for sharing the public keys of users that want to share files.

## Self-hosting
If for any reason you want to self-host a server, this is possible but not well supported (subject to change). use `cargo run --bin server` to compile and run the server binary, it will automatically create new encryption keys. The public key signature will get printed to the terminal, save that -- you need it later. An http tunnel is also needed, such as ngrok, http traffic must be redirected to port 8000 on the server. The server/http tunnel should be run from a publicly reachable location, otherwise clients will likely be unable to connect to it.
To make a client use the server, inside src/constants.rs, modify the SERVER_PUBLIC_KEY string to be the public key that was printed when the server started. Then, modify SERVER_URL to the url of the http tunnel that forwards traffic to port 8000 on the server (or, pass in SERVER_URL=http://your-url as a command-line argument every time you run pigeon)
> Note: when self hosting, only clients on the same local network or using the same self-hosted server will be able to see/connect to each other
