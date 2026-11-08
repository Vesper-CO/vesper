# vesper-rust

Builds the Rust client that Vesper's **Rust** instances run: Horizon's client with the name, window title,
logo, version text and settings changed to Vesper, and everything that talked to Horizon's servers turned off
(the "playing now" ping, the cosmetics service, Discord presence).

Based on [Horizon](https://github.com/horizon-rust/horizon) (MIT). See `NOTICE.md` and `LICENSE-HORIZON`.

## One-time setup

1. Create a public GitHub repository named `vesper-rust` under your account and push these files to it.
2. Make a signing key (Git Bash or any shell with OpenSSL):
   ```
   openssl genpkey -algorithm ed25519 -out rust-signing.pem
   openssl pkey -in rust-signing.pem -pubout -outform DER | tail -c 32 | base64
   ```
   The second command prints the **public key**. Keep `rust-signing.pem` private and never commit it.
3. In the repository, go to Settings, Secrets and variables, Actions, and add a secret named
   `RUST_SIGNING_KEY` containing the whole text of `rust-signing.pem`.
4. In Vesper's `src-tauri/src/rust.rs`, paste the public key between the quotes of `VESPER_KEY`.
5. In the repository's **Actions** tab, run **Build Vesper Rust client**. The first build is slow (30 to 60
   minutes). Then it runs every Monday on its own.

Until a build exists, Vesper falls back to Horizon's official build.

## If a build fails

`rebrand.py` stops the build when Horizon has changed code it edits, so a half-renamed client is never
published. The log lists which edit no longer matches; fix that line in `rebrand.py`.
