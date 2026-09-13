# ROPDS Telegram Bot

[![CI](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml)
[![Release](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/release.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Telegram-бот для поиска и скачивания книг из самодостаточной библиотеки [ROPDS](https://github.com/dshein-alt/ropds) через протокол OPDS 2.0.

[🇬🇧 English version](README.md)

## ✨ Возможности

- 🔍 Поиск книг по названию, автору или жанру
- 📚 Просмотр новых поступлений, авторов и жанров
- 📥 Скачивание книг прямо в чат Telegram (до 50 МБ)
- 🔗 Прямая ссылка для больших файлов
- 🖼️ Поддержка обложек
- 🔐 Поддержка HTTP Basic Auth для защищённых инсталляций ROPDS
- 🚀 Лёгкий одиночный бинарник (~10 МБ)
- 🐳 Готов к Docker

## 🚀 Быстрый старт

### Из бинарников

Скачайте последний релиз для вашей платформы на [странице GitHub Releases](https://github.com/sergej19882906/ropds-telegram-bot/releases).

### Запуск Windows-бинарника

Скачайте подходящий архив из GitHub Releases:

- `x86_64-pc-windows-msvc` — обычная 64-битная Windows для Intel/AMD;
- `aarch64-pc-windows-msvc` — Windows ARM64.

Распакуйте архив, создайте рядом с `ropds-telegram-bot.exe` файл `.env` и добавьте настройки:

```env
BOT_TOKEN=ваш_telegram_bot_token
ROPDS_URL=http://127.0.0.1:8081
RUST_LOG=info
```

Запустите бота из PowerShell:

```powershell
cd C:\Apps\ropds-telegram-bot
.\ropds-telegram-bot.exe
```

Если Windows заблокировал скачанный файл, разблокируйте его:

```powershell
Unblock-File .\ropds-telegram-bot.exe
```

Для остановки нажмите `Ctrl+C`. Для запуска в фоне используйте:

```powershell
Start-Process -FilePath "C:\Apps\ropds-telegram-bot\ropds-telegram-bot.exe" `
  -WorkingDirectory "C:\Apps\ropds-telegram-bot"
```

Файл `.env` содержит секреты и не должен загружаться на GitHub.

### Из исходников

```bash
# Требования: Rust 1.75+
git clone https://github.com/sergej19882906/ropds-telegram-bot.git
cd ropds-telegram-bot

cp .env.example .env
# Отредактируйте .env, указав BOT_TOKEN и ROPDS_URL

cargo run --release
```

Если нужно собрать проект без запуска, используйте `cargo build --release`, после чего можно запустить бинарник из `target/release/ropds-telegram-bot`.

Workflow релизов настроен для сборки Linux x86_64 (GNU и MUSL), Linux ARM64, Windows x86_64, Windows ARM64, а также macOS Intel и Apple Silicon. Бинарники для Linux публикуются в архивах `.tar.gz`, для Windows и macOS — в архивах `.zip`.

### Запуск Linux-бинарника через systemd

Определите архитектуру системы:

```bash
uname -m
```

Скачайте подходящий бинарник из GitHub Releases (`x86_64` для `x86_64`, `aarch64` для `aarch64`) и установите его:

```bash
sudo useradd --system --home /opt/ropds-telegram-bot --shell /usr/sbin/nologin ropds
sudo mkdir -p /opt/ropds-telegram-bot
sudo cp ropds-telegram-bot /opt/ropds-telegram-bot/
sudo chown -R ropds:ropds /opt/ropds-telegram-bot
sudo chmod 755 /opt/ropds-telegram-bot/ropds-telegram-bot
```

Создайте файл `/opt/ropds-telegram-bot/.env`:

```env
BOT_TOKEN=ваш_telegram_bot_token
ROPDS_URL=http://127.0.0.1:8081
RUST_LOG=info
```

Ограничьте доступ к конфигурации:

```bash
sudo chown ropds:ropds /opt/ropds-telegram-bot/.env
sudo chmod 600 /opt/ropds-telegram-bot/.env
```

Создайте `/etc/systemd/system/ropds-telegram-bot.service`:

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

Включите автозапуск и запустите сервис:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ropds-telegram-bot
sudo systemctl status ropds-telegram-bot
```

Просмотр логов:

```bash
sudo journalctl -u ropds-telegram-bot -f
```

### Через Docker

Создайте файл конфигурации:

```bash
cp .env.example .env
# Отредактируйте .env, указав BOT_TOKEN и ROPDS_URL
```

Соберите и запустите контейнер через Docker Compose:

```bash
docker compose up -d --build
```

Проверьте состояние контейнера и логи:

```bash
docker compose ps
docker compose logs -f ropds-bot
```

Остановите контейнер:

```bash
docker compose down
```

Чтобы использовать готовый образ из GitHub Container Registry вместо локальной сборки, укажите `image` в `docker-compose.yml`:

```yaml
services:
  ropds-bot:
    image: ghcr.io/sergej19882906/ropds-telegram-bot:latest
    env_file:
      - .env
    restart: unless-stopped
```

После этого запустите контейнер:

```bash
docker compose up -d
```

### GitHub Container Registry

При создании релизного тега образ автоматически публикуется в GitHub Container Registry:

```text
ghcr.io/sergej19882906/ropds-telegram-bot
```

Войти в реестр и скачать последнюю версию образа:

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u YOUR_GITHUB_USERNAME --password-stdin
docker pull ghcr.io/sergej19882906/ropds-telegram-bot:latest
docker run --rm --env-file .env ghcr.io/sergej19882906/ropds-telegram-bot:latest
```

Опубликованный образ поддерживает `linux/amd64` и `linux/arm64`; Docker автоматически выбирает подходящую архитектуру. Токен должен иметь право на чтение packages. Workflow использует `GITHUB_TOKEN` с правом записи packages и публикует образ при отправке тега версии, например `v0.1.2`.

## ⚙️ Конфигурация

Все настройки загружаются из переменных окружения или файла `.env`:

| Переменная | Обязательна | По умолчанию | Описание |
|---|---|---|---|
| `BOT_TOKEN` | ✅ | — | Токен бота от [@BotFather](https://t.me/BotFather) |
| `ROPDS_URL` | ❌ | `http://localhost:8081` | URL сервера ROPDS |
| `ROPDS_USER` | ❌ | — | Имя пользователя HTTP Basic Auth |
| `ROPDS_PASSWORD` | ❌ | — | Пароль HTTP Basic Auth |
| `ALLOWED_USER_IDS` | ❌ | — | Разрешённые Telegram user ID через запятую; пустое значение разрешает всех |
| `ALLOW_ALL_USERS` | ❌ | auto | `false` требует `ALLOWED_USER_IDS`; пустое/`true` оставляет доступ для всех |
| `MAX_BOOK_SIZE_MB` | ❌ | `50` | Максимальный размер книги для отправки в Telegram |
| `MAX_COVER_SIZE_MB` | ❌ | `5` | Максимальный размер обложки |
| `MAX_FEED_SIZE_MB` | ❌ | `10` | Максимальный размер JSON-ответа OPDS |
| `MAX_CONCURRENT_DOWNLOADS` | ❌ | `2` | Параллельные скачивания книг |
| `REQUEST_COOLDOWN_SECS` | ❌ | `2` | Пауза между командами/кнопками одного пользователя |
| `BOOKS_PER_PAGE` | ❌ | `5` | Сколько книг показать до кнопки «Показать ещё» |
| `BOT_READY_FILE` | ❌ | — | Файл после успешного Telegram `getMe` (для Docker healthcheck) |
| `RUST_LOG` | ❌ | `info` | Уровень логирования |

Проект намеренно использует `rustls` вместо OpenSSL, поэтому для сборки достаточно обычного Rust toolchain. Лимиты книги, обложки, OPDS-ответа, параллельных загрузок и паузы между запросами задаются через env; значения по умолчанию — 50 МБ / 5 МБ / 10 МБ / 2 загрузки / 2 секунды. Для production задайте `ALLOWED_USER_IDS` или `ALLOW_ALL_USERS=false`. Результаты поиска показываются страницами с кнопкой «Показать ещё».

## 📖 Команды

| Команда | Описание |
|---|---|
| `/start` | Показать справку |
| `/search <запрос>` | Поиск книг |
| `/recent` | Показать новые поступления |
| `/authors` | Просмотр списка авторов |
| `/genres` | Просмотр списка жанров |

## 🏗 Разработка

```bash
cargo check          # Проверка кода
cargo build          # Сборка debug
cargo build --release # Сборка release
cargo test           # Тесты
cargo fmt            # Форматирование
cargo clippy         # Линтинг
```

### Кросс-компиляция релизов

Workflow GitHub Releases собирает следующие Rust targets:

| Платформа | Rust target | Архив |
|---|---|---|
| Linux x86_64 (GNU) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux x86_64 (MUSL) | `x86_64-unknown-linux-musl` | `.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `.zip` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `.zip` |
| macOS Intel | `x86_64-apple-darwin` | `.zip` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.zip` |

В Windows Linux targets можно собрать с помощью [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) и Zig:

```powershell
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-unknown-linux-gnu
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-gnu
```

Файл `.env` читается при запуске бинарника и не встраивается в него во время компиляции.

### Настройка VS Code

В `.vscode/` лежат рекомендуемые настройки. Установите рекомендуемые расширения и нажмите `F5` для запуска отладки.

## 📄 Лицензия

Проект лицензирован на выбор под:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE.txt))
- MIT license ([LICENSE-MIT](LICENSE-MIT.txt))

## 🤝 Участие в разработке

Contributions welcome! Открывайте issue или присылайте pull request.

## 🙏 Благодарности

- [ROPDS](https://github.com/dshein-alt/ropds) — сервер библиотеки, с которым работает бот
- [teloxide](https://github.com/teloxide/teloxide) — элегантный фреймворк для Telegram-ботов на Rust
