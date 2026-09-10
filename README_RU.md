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

### Через Docker

```bash
cp .env.example .env
# Отредактируйте .env

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

Токен должен иметь право на чтение packages. Workflow использует `GITHUB_TOKEN` с правом записи packages и публикует образ при отправке тега версии, например `v0.1.2`.

## ⚙️ Конфигурация

Все настройки загружаются из переменных окружения или файла `.env`:

| Переменная | Обязательна | По умолчанию | Описание |
|---|---|---|---|
| `BOT_TOKEN` | ✅ | — | Токен бота от [@BotFather](https://t.me/BotFather) |
| `ROPDS_URL` | ❌ | `http://localhost:8081` | URL сервера ROPDS |
| `ROPDS_USER` | ❌ | — | Имя пользователя HTTP Basic Auth |
| `ROPDS_PASSWORD` | ❌ | — | Пароль HTTP Basic Auth |
| `ALLOWED_USER_IDS` | ❌ | — | Разрешённые Telegram user ID через запятую; пустое значение разрешает всех |
| `RUST_LOG` | ❌ | `info` | Уровень логирования |

Проект намеренно использует `rustls` вместо OpenSSL, поэтому для сборки достаточно обычного Rust toolchain. Размер книги ограничен 50 МБ, размер обложки — 5 МБ, одновременно выполняются не более двух скачиваний. Для production рекомендуется задать `ALLOWED_USER_IDS`.

## 📖 Команды

| Команда | Описание |
|---|---|
| `/start` | Показать справку |
| `/help` | Показать справку |
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
