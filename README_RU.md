# ROPDS Telegram Bot

[![CI](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml/badge.svg)](https://github.com/sergej19882906/ropds-telegram-bot/actions/workflows/ci.yml)
sergej19882906[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Telegram-бот для поиска и скачивания книг из самодостаточной библиотеки [ROPDS](https://github.com/dshein-alt/ropds) через протокол OPDS 2.0.

[🇬🇧 English version](README.md)

## ✨ Возможности

- 🔍 Поиск книг по названию, автору или жанру
- 📥 Скачивание книг прямо в чат Telegram (до 50 МБ)
- 🔗 Прямая ссылка для больших файлов
- 🖼️ Поддержка обложек
- 🔐 Поддержка HTTP Basic Auth для защищённых инсталляций ROPDS
- 🚀 Лёгкий одиночный бинарник (~10 МБ)
- 🐳 Готов к Docker

## 🚀 Быстрый старт

### Из бинарников

Скачайте последний релиз для вашей платформы из [Releases](../../releases).

### Из исходников

```bash
# Требования: Rust 1.75+
git clone https://github.com/sergej19882906/ropds-telegram-bot.git
cd ropds-telegram-bot

cp .env.example .env
# Отредактируйте .env, указав BOT_TOKEN и ROPDS_URL

cargo run --release
```

### Через Docker

```bash
cp .env.example .env
# Отредактируйте .env

docker compose up -d
```

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

Размер книги ограничен 50 МБ, размер обложки — 5 МБ, одновременно выполняются не более двух скачиваний. Для production рекомендуется задать `ALLOWED_USER_IDS`.

## 📖 Команды

| Команда | Описание |
|---|---|
| `/start` | Показать справку |
| `/help` | Показать справку |
| `/search <запрос>` | Поиск книг |

## 🏗 Разработка

```bash
cargo check          # Проверка кода
cargo build          # Сборка debug
cargo build --release # Сборка release
cargo test           # Тесты
cargo fmt            # Форматирование
cargo clippy         # Линтинг
```

### Настройка VS Code

В `.vscode/` лежат рекомендуемые настройки. Установите рекомендуемые расширения и нажмите `F5` для запуска отладки.

## 📄 Лицензия

Проект лицензирован на выбор под:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

## 🤝 Участие в разработке

Contributions welcome! Открывайте issue или присылайте pull request.

## 🙏 Благодарности

- [ROPDS](https://github.com/dshein-alt/ropds) — сервер библиотеки, с которым работает бот
- [teloxide](https://github.com/teloxide/teloxide) — элегантный фреймворк для Telegram-ботов на Rust
