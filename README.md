# ROPDS Telegram Bot

[![CI](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml)
[![Release](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/release.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Telegram bot for searching and downloading books from a self-hosted [ROPDS](https://github.com/dshein-alt/ropds) e-book library via the OPDS 2.0 protocol.

[🇷🇺 Русская версия](README_RU.md)

## ✨ Features

- 🔍 Search books by title, author, or genre
- 📚 Browse recent additions, authors, and genres
- 📥 Download books directly to Telegram chat (up to 50 MB)
- 🔗 Get a direct link for larger files
- 🖼️ Cover image support
- 🔐 HTTP Basic Auth support for secured ROPDS instances
- 🚀 Lightweight single binary (~10 MB)
- 🐳 Docker-ready

## 🚀 Quick start

### From binaries

Download the latest release for your platform from the [GitHub Releases page](https://github.com/sergej19882906/ropds-telegram-bot/releases).

### From source

```bash
# Requirements: Rust 1.75+
git clone https://github.com/sergej19882906/ropds-telegram-bot.git
cd ropds-telegram-bot

cp .env.example .env
# Edit .env with your BOT_TOKEN and ROPDS_URL

cargo run --release
```

If you prefer to build without running the app, use `cargo build --release` and then start the binary from `target/release/ropds-telegram-bot`.

The release workflow is configured for Linux x86_64 (GNU and MUSL), Linux ARM64, Windows x86_64, Windows ARM64, and macOS Intel and Apple Silicon. Linux binaries are published as `.tar.gz` archives; Windows and macOS binaries are published as `.zip` archives.

### With Docker

```bash
cp .env.example .env
# Edit .env

docker compose up -d
```

### GitHub Container Registry

Release tags automatically publish the image to GitHub Container Registry:

```text
ghcr.io/sergej19882906/ropds-telegram-bot
```

Log in and pull the latest image:

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u YOUR_GITHUB_USERNAME --password-stdin
docker pull ghcr.io/sergej19882906/ropds-telegram-bot:latest
docker run --rm --env-file .env ghcr.io/sergej19882906/ropds-telegram-bot:latest
```

The token must have permission to read packages. The workflow uses `GITHUB_TOKEN` with package write permission to publish images when a version tag such as `v0.1.2` is pushed.

## ⚙️ Configuration

All settings are loaded from environment variables or a `.env` file:

| Variable | Required | Default | Description |
|---|---|---|---|
| `BOT_TOKEN` | ✅ | — | Telegram bot token from [@BotFather](https://t.me/BotFather) |
| `ROPDS_URL` | ❌ | `http://localhost:8081` | ROPDS server URL |
| `ROPDS_USER` | ❌ | — | HTTP Basic Auth username |
| `ROPDS_PASSWORD` | ❌ | — | HTTP Basic Auth password |
| `ALLOWED_USER_IDS` | ❌ | — | Comma-separated allowed Telegram user IDs; empty allows all users |
| `RUST_LOG` | ❌ | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

The project intentionally uses `rustls` instead of OpenSSL, so a standard Rust toolchain is enough for local builds. Book downloads are limited to 50 MB, covers to 5 MB, and at most two downloads run concurrently. Set `ALLOWED_USER_IDS` for production deployments.

## 📖 Commands

| Command | Description |
|---|---|
| `/start` | Show help |
| `/help` | Show help |
| `/search <query>` | Search books |
| `/recent` | Show recent additions |
| `/authors` | Browse authors |
| `/genres` | Browse genres |

## 🏗 Development

```bash
cargo check          # Check code
cargo build          # Build debug
cargo build --release # Build release
cargo test           # Run tests
cargo fmt            # Format code
cargo clippy         # Lint
```

### Cross-platform release builds

The GitHub release workflow builds the following Rust targets:

| Platform | Rust target | Archive |
|---|---|---|
| Linux x86_64 (GNU) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux x86_64 (MUSL) | `x86_64-unknown-linux-musl` | `.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `.zip` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `.zip` |
| macOS Intel | `x86_64-apple-darwin` | `.zip` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.zip` |

On Windows, Linux targets can be built with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) and Zig:

```powershell
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-unknown-linux-gnu
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-gnu
```

The `.env` file is read when the binary starts; it is not embedded into the binary during compilation.

### VS Code setup

The project includes recommended VS Code settings in `.vscode/`. Install the recommended extensions and press `F5` to start debugging.

## 🐳 Docker

Pre-built images will be available at `ghcr.io/sergej19882906/ropds-telegram-bot` after the first release.

```bash
docker build -t ropds-telegram-bot .
docker run --env-file .env ropds-telegram-bot
```

## 📄 License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

## 🤝 Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## 🙏 Acknowledgments

- [ROPDS](https://github.com/dshein-alt/ropds) — the e-book library server this bot integrates with
- [teloxide](https://github.com/teloxide/teloxide) — elegant Telegram bots framework for Rust
