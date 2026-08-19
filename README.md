# Pigeon
> File sharing app that aims to be similar to localsend, but cross-network

## Available installation methods
- Go to the releases tab and download the binary for your operating system, or use the installation script (shell script for Linux, powershell script for Windows, homebrew tap for macOS)
- Arch Linux: AUR package coming soonish (~~before August 15th hopefully~~ damn I missed my deadline)
- ~~crates.io: might be published by August 15th as well~~ didn't do this either, turns out its kinda pointless
- Android support: maybe one day, ios probably never cause pain

> You can also build from source by installing the rust toolchain if you don't have it already, cloning the repo, and running cargo build --release from the project root

## Usage
Listen/receive files: `pigeon`

Send files/folders: `pigeon /path/to/file/or/folder`

Change name: `pigeon --change-name`
> Running pigeon in any way for the first time will automatically generate encryption keys and prompt you to select a username

Re-generating encryption keys is not natively supported, but you can manually delete the keys inside the app's data directory. On windows its `%appdata%/pigeon`, on macos its `/Library/Application Support/pigeon`, on linux its `~/.local/share/pigeon`. You can edit the username file inside the data folder as well, but this does not update the server side registry, so using --change-name is the preferred method.

## Uninstalling
Linux: Delete pigeon and pigeon-update from `$CARGO_HOME/bin`. If `CARGO_HOME` isn't set, check `~/.cargo/bin`. To also delete app data, delete `~/.local/share/pigeon`.
macOS: Delete pigeon and pigeon-update from `$CARGO_HOME/bin`. If `CARGO_HOME` isn't set, check `~/.cargo/bin`. To also delete app data, delete `/Library/Application Support/pigeon`
Windows: Delete pigeon.exe and pigeon-update.exe from `%CARGO_HOME%\bin`. If `CARGO_HOME` isn't set, check `C:\Users\<user>\.cargo\bin\)` where <user> is your user. To also delete app data, delete `{FOLDERID_LocalAppData}\pigeon\data` or if that isn't set, `C:\Users\<user>\AppData\Local\pigeon\data` wgere <user> is your user.

## Data privacy and encryption
The names and types of files you send and receive and their contents are end-to-end encrypted with ed25519 and the server never sees any verison of them. Connections with the server are encrypted with tls. Only your username and public key is ever shared with the server. The server is only used for sharing the public keys of users that want to share files.

## Self-hosting
If for any reason you want to self-host a server, this is possible but not well supported (subject to change). use `cargo run --bin server` to compile and run the server binary, it will automatically create new encryption keys. The public key signature will get printed to the terminal, save that -- you need it later. An http tunnel is also needed, such as ngrok, http traffic must be redirected to port 8000 on the server. The server/http tunnel should be run from a publicly reachable location, otherwise clients will likely be unable to connect to it.
To make a client use the server, inside `src/client/constants.rs`, modify the SERVER_PUBLIC_KEY string to be the public key that was printed when the server started. Then, modify SERVER_URL to the url of the http tunnel that forwards traffic to port 8000 on the server (or, pass in SERVER_URL=http://your-url as a command-line argument every time you run pigeon). You should also modify ADMIN_KEYS to include your public key, otherwise you won't be able to use download-db and inject-db which are useful for restoring state after server upgrades/restarts.

When self hosting from a http server (not https), it is recommended but not required to enable custom https. To do this, generate a tls certificate/key pair, copy the certificate to rootCA.pem on all the clients, and have both cert.pem (certificate) and key.pem (key) in the directory the server gets run from. The server needs to be run with USE_CUSTOM_HTTPS=true, and so do all the clients. Note that client-to-client transfers, which includes names and types of files and their contents, are always encrypted regardless of whether https is set up.
> Note: when self hosting, only clients on the same local network or using the same self-hosted server will be able to see/connect to each other
