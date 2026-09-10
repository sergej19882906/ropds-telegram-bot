# ROPDS Telegram Bot

[![CI](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml)
[![Release](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/release.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Telegram bot for searching and downloading books from a self-hosted [ROPDS](https://github.com/dshein-alt/ropds) e-book library via the OPDS 2.0 protocol.

[🇷🇺 Русская версия](README_RU.md)

## ✨ Features

- 🔍 Search books by title, author, or genre
- 📥 Download books directly to Telegram chat (up to 50 MB)
- 🔗 Direct link for larger files
- 🖼️ Cover images support
- 🔐 HTTP Basic Auth support for secured ROPDS instances
- 🚀 Lightweight single binary (~10 MB)
- 🐳 Docker-ready

## 🚀 Quick start

### From binaries

Download the latest release for your platform from [Releases](../../releases).

### From source

```bash
# Requirements: Rust 1.75+
git clone https://github.com/sergej19882906/ropds-telegram-bot.git
cd ropds-telegram-bot

cp .env.example .env
# Edit .env with your BOT_TOKEN and ROPDS_URL

cargo run --release
```

### With Docker

```bash
cp .env.example .env
# Edit .env

docker compose up -d
```

## ⚙️ Configuration

All settings are loaded from environment variables or `.env` file:

| Variable | Required | Default | Description |
|---|---|---|---|
| `BOT_TOKEN` | ✅ | — | Telegram bot token from [@BotFather](https://t.me/BotFather) |
| `ROPDS_URL` | ❌ | `http://localhost:8081` | ROPDS server URL |
| `ROPDS_USER` | ❌ | — | HTTP Basic Auth username |
| `ROPDS_PASSWORD` | ❌ | — | HTTP Basic Auth password |
| `ALLOWED_USER_IDS` | ❌ | — | Comma-separated allowed Telegram user IDs; empty allows all users |
| `RUST_LOG` | ❌ | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

Book downloads are limited to 50 MB, covers to 5 MB, and at most two downloads run concurrently. Set `ALLOWED_USER_IDS` for production deployments.

## 📖 Commands

| Command | Description |
|---|---|
| `/start` | Show help |
| `/help` | Show help |
| `/search <query>` | Search for books |

## 🏗 Development

```bash
cargo check          # Check code
cargo build          # Build debug
cargo build --release # Build release
cargo test           # Run tests
cargo fmt            # Format code
cargo clippy         # Lint
```

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

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🤝 Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## 🙏 Acknowledgments

- [ROPDS](https://github.com/dshein-alt/ropds) — the e-book library server this bot integrates with
- [teloxide](https://github.com/teloxide/teloxide) — elegant Telegram bots framework for Rust
