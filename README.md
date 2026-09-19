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

### Run a Windows binary

Download the matching archive from GitHub Releases:

- `x86_64-pc-windows-msvc` for standard 64-bit Intel/AMD Windows;
- `aarch64-pc-windows-msvc` for Windows on ARM64.

Extract the archive, create a `.env` file next to `ropds-telegram-bot.exe`, and add your settings:

```env
BOT_TOKEN=your_telegram_bot_token
ROPDS_URL=http://127.0.0.1:8081
RUST_LOG=info
```

Start the bot from PowerShell:

```powershell
cd C:\Apps\ropds-telegram-bot
.\ropds-telegram-bot.exe
```

If Windows blocks the downloaded executable, unblock it before starting:

```powershell
Unblock-File .\ropds-telegram-bot.exe
```

Stop the bot with `Ctrl+C`. To start it in the background:

```powershell
Start-Process -FilePath "C:\Apps\ropds-telegram-bot\ropds-telegram-bot.exe" `
  -WorkingDirectory "C:\Apps\ropds-telegram-bot"
```

The `.env` file contains secrets and must not be committed to GitHub.

### Run a Linux binary with systemd

Check the system architecture:

```bash
uname -m
```

Download the matching binary from GitHub Releases (`x86_64` for `x86_64`, `aarch64` for `aarch64`) and install it:

```bash
sudo useradd --system --home /opt/ropds-telegram-bot --shell /usr/sbin/nologin ropds
sudo mkdir -p /opt/ropds-telegram-bot
sudo cp ropds-telegram-bot /opt/ropds-telegram-bot/
sudo chown -R ropds:ropds /opt/ropds-telegram-bot
sudo chmod 755 /opt/ropds-telegram-bot/ropds-telegram-bot
```

Create `/opt/ropds-telegram-bot/.env`:

```env
BOT_TOKEN=your_telegram_bot_token
ROPDS_URL=http://127.0.0.1:8081
RUST_LOG=info
```

Restrict access to the configuration file:

```bash
sudo chown ropds:ropds /opt/ropds-telegram-bot/.env
sudo chmod 600 /opt/ropds-telegram-bot/.env
```

Create `/etc/systemd/system/ropds-telegram-bot.service`:

```ini
[Unit]
Description=ROPDS Telegram Bot
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=ropds
Group=ropds
WorkingDirectory=/opt/ropds-telegram-bot
EnvironmentFile=/opt/ropds-telegram-bot/.env
ExecStart=/opt/ropds-telegram-bot/ropds-telegram-bot
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

Enable automatic startup and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ropds-telegram-bot
sudo systemctl status ropds-telegram-bot
```

View logs:

```bash
sudo journalctl -u ropds-telegram-bot -f
```

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

Create the configuration file:

```bash
cp .env.example .env
# Edit .env and set BOT_TOKEN and ROPDS_URL
```

Build and start the container with Docker Compose:

```bash
docker compose up -d --build
```

Check the container status and logs:

```bash
docker compose ps
docker compose logs -f ropds-bot
```

Stop the container:

```bash
docker compose down
```

To use the pre-built image from GitHub Container Registry instead of building locally, set `image` in `docker-compose.yml`:

```yaml
services:
  ropds-bot:
    image: ghcr.io/sergej19882906/ropds-telegram-bot:latest
    env_file:
      - .env
    restart: unless-stopped
```

Then start it:

```bash
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

The published image supports both `linux/amd64` and `linux/arm64`; Docker selects the matching architecture automatically. The token must have permission to read packages. The workflow uses `GITHUB_TOKEN` with package write permission to publish images when a version tag such as `v0.1.2` is pushed.

## ⚙️ Configuration

All settings are loaded from environment variables or a `.env` file:

| Variable | Required | Default | Description |
|---|---|---|---|
| `BOT_TOKEN` | ✅ | — | Telegram bot token from [@BotFather](https://t.me/BotFather) |
| `ROPDS_URL` | ❌ | `http://localhost:8081` | ROPDS server URL |
| `ROPDS_USER` | ❌ | — | HTTP Basic Auth username |
| `ROPDS_PASSWORD` | ❌ | — | HTTP Basic Auth password |
| `REDIS_URL` | ❌ | `redis://127.0.0.1:6379` | Redis connection URL for state management |
| `ALLOWED_USER_IDS` | ❌ | — | Comma-separated allowed Telegram user IDs; empty allows all users |
| `ALLOW_ALL_USERS` | ❌ | auto | `false` requires `ALLOWED_USER_IDS`; empty/true keeps allow-all |
| `MAX_BOOK_SIZE_MB` | ❌ | `50` | Maximum book size to send to Telegram |
| `MAX_COVER_SIZE_MB` | ❌ | `5` | Maximum cover image size |
| `MAX_FEED_SIZE_MB` | ❌ | `10` | Maximum OPDS JSON response size |
| `MAX_CONCURRENT_DOWNLOADS` | ❌ | `2` | Parallel book downloads |
| `REQUEST_COOLDOWN_SECS` | ❌ | `2` | Per-user command/callback cooldown |
| `BOOKS_PER_PAGE` | ❌ | `5` | Books shown before the “show more” button |
| `BOOKS_TIMEOUT_SECS` | ❌ | `180` | Timeout for books search/feed requests |
| `FEED_TIMEOUT_SECS` | ❌ | `60` | Timeout for general OPDS feed requests |
| `COVER_TIMEOUT_SECS` | ❌ | `15` | Timeout for cover image requests |
| `DOWNLOAD_TIMEOUT_SECS` | ❌ | `120` | Timeout for book downloads |
| `BOT_READY_FILE` | ❌ | — | Written after a successful Telegram `getMe` (used by Docker healthcheck) |
| `RUST_LOG` | ❌ | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

The project intentionally uses `rustls` instead of OpenSSL, so a standard Rust toolchain is enough for local builds. Book downloads, cover size, feed size, concurrency, and cooldown are configurable; defaults are 50 MB / 5 MB / 10 MB / 2 downloads / 2 seconds. Set `ALLOWED_USER_IDS` (or `ALLOW_ALL_USERS=false`) for production deployments. Search results are shown in pages with a “show more” button.

## 📖 Commands

| Command | Description |
|---|---|
| `/start` | Show help |
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

Pre-built images are available at `ghcr.io/sergej19882906/ropds-telegram-bot` after each version tag release.

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
