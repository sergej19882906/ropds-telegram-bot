use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use teloxide::prelude::*;
use teloxide::types::{InputFile, KeyboardButton, KeyboardMarkup, ReplyParameters};
use teloxide::utils::command::BotCommands;

use crate::ropds::{Book, DownloadContext, RopdsClient};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Команды бота:")]
pub enum Cmd {
    #[command(description = "показать справку")]
    Start,
    #[command(description = "поиск книги: /search <запрос>")]
    Search(String),
    #[command(description = "недавние поступления")]
    Recent,
    #[command(description = "список авторов")]
    Authors,
    #[command(description = "список жанров")]
    Genres,
}

pub struct BotState {
    pub ropds: RopdsClient,
    pub download_cache: DashMap<u64, CachedDownload>,
    pub navigation_cache: DashMap<u64, NavigationTarget>,
    pub next_id: AtomicU64,
    pub allowed_user_ids: HashSet<u64>,
    pub last_activity: DashMap<u64, Instant>,
    pub downloads: Arc<tokio::sync::Semaphore>,
}

pub type SharedState = Arc<BotState>;

#[derive(Clone)]
pub struct CachedDownload {
    pub owner_id: u64,
    pub context: DownloadContext,
}

#[derive(Clone)]
pub struct NavigationTarget {
    pub owner_id: u64,
    pub target: String,
}

fn message_user_id(msg: &Message) -> Option<u64> {
    msg.from.as_ref().map(|user| user.id.0)
}

fn callback_user_id(q: &teloxide::types::CallbackQuery) -> u64 {
    q.from.id.0
}

fn is_allowed(state: &SharedState, user_id: u64) -> bool {
    state.allowed_user_ids.is_empty() || state.allowed_user_ids.contains(&user_id)
}

fn allow_request(state: &SharedState, user_id: u64) -> bool {
    let now = Instant::now();
    match state.last_activity.entry(user_id) {
        dashmap::mapref::entry::Entry::Occupied(mut entry) => {
            if now.duration_since(*entry.get()) < Duration::from_secs(2) {
                return false;
            }
            entry.insert(now);
        }
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            entry.insert(now);
        }
    }
    true
}

fn main_menu() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new("/search"),
            KeyboardButton::new("/recent"),
        ],
        vec![
            KeyboardButton::new("/authors"),
            KeyboardButton::new("/genres"),
        ],
        vec![KeyboardButton::new("/start")],
    ])
    .persistent()
    .resize_keyboard()
}

pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Cmd,
    state: SharedState,
) -> ResponseResult<()> {
    let Some(user_id) = message_user_id(&msg) else {
        return Ok(());
    };
    if !is_allowed(&state, user_id) {
        tracing::warn!(user_id, "Rejected unauthorized command");
        return Ok(());
    }
    if !allow_request(&state, user_id) {
        bot.send_message(msg.chat.id, "Слишком много запросов. Попробуйте позже.")
            .await?;
        return Ok(());
    }

    match cmd {
        Cmd::Start => {
            let text = format!(
                "📚 *Бот библиотеки ROPDS*\n\n{}\n\nИспользуйте `/search <запрос>` для поиска книг\\.",
                escape_md(&Cmd::descriptions().to_string())
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .reply_markup(main_menu())
                .await?;
        }
        Cmd::Search(query) => {
            if query.trim().is_empty() {
                bot.send_message(msg.chat.id, "⚠️ Укажите запрос\\. Пример: `/search Дюна`")
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .await?;
                return Ok(());
            }
            let result = state.ropds.search(&query).await;
            send_books_with_covers(&bot, msg.chat.id, user_id, &state, &result).await?;
        }
        Cmd::Recent => {
            let _ = bot
                .send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
                .await;
            let result = state.ropds.get_recent_books().await;
            send_books_with_covers(&bot, msg.chat.id, user_id, &state, &result).await?;
        }
        Cmd::Authors | Cmd::Genres => {
            let _ = bot
                .send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
                .await;

            let path = if matches!(cmd, Cmd::Authors) {
                "/opds/v2/authors/?lang=ru"
            } else {
                "/opds/v2/genres/?lang=ru"
            };

            match state.ropds.get_navigation(path).await {
                Ok(items) if items.is_empty() => {
                    bot.send_message(msg.chat.id, "😔 Список пуст\\.")
                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                        .await?;
                }
                Ok(items) => {
                    let mut rows: Vec<Vec<teloxide::types::InlineKeyboardButton>> = Vec::new();
                    for item in items {
                        let navigation_id = state.next_id.fetch_add(1, Ordering::SeqCst);
                        state.navigation_cache.insert(
                            navigation_id,
                            NavigationTarget {
                                owner_id: user_id,
                                target: item
                                    .href
                                    .clone()
                                    .unwrap_or_else(|| format!("search:{}", item.title)),
                            },
                        );
                        rows.push(vec![teloxide::types::InlineKeyboardButton::callback(
                            item.title.clone(),
                            format!("nav:{navigation_id}"),
                        )]);
                    }
                    let markup = teloxide::types::InlineKeyboardMarkup::new(rows);
                    let title = if matches!(cmd, Cmd::Authors) {
                        "Авторы"
                    } else {
                        "Жанры"
                    };

                    bot.send_message(
                        msg.chat.id,
                        format!("📂 *{}* \\(выберите для поиска\\):", title),
                    )
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .reply_markup(markup)
                    .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Navigation failed");
                    bot.send_message(msg.chat.id, "❌ Ошибка при получении списка\\.")
                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                        .await?;
                }
            }
        }
    }
    Ok(())
}

async fn send_books_with_covers(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    owner_id: u64,
    state: &SharedState,
    result: &anyhow::Result<Vec<Book>>,
) -> ResponseResult<()> {
    match result {
        Ok(books) if books.is_empty() => {
            bot.send_message(chat_id, "😔 Ничего не найдено\\.")
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
        Ok(books) => {
            for book in books.iter().take(10) {
                let id = state.next_id.fetch_add(1, Ordering::SeqCst);
                state.download_cache.insert(
                    id,
                    CachedDownload {
                        owner_id,
                        context: DownloadContext {
                            url: book.url.clone(),
                            title: book.title.clone(),
                            author: book.author.clone(),
                            cover_url: book.cover_url.clone(),
                        },
                    },
                );

                let keyboard = teloxide::types::InlineKeyboardMarkup::new(vec![vec![
                    teloxide::types::InlineKeyboardButton::callback(
                        "📥 Скачать",
                        format!("dl:{id}"),
                    ),
                ]]);

                if let Some(cover_url) = &book.cover_url {
                    if let Some(cover_bytes) = state.ropds.download_cover(cover_url).await {
                        let caption = format!(
                            "📚 *{}*\n👤 *Автор:* {}\n\nНажмите кнопку ниже, чтобы скачать\\.",
                            escape_md(&book.title),
                            escape_md(&book.author)
                        );
                        let photo = InputFile::memory(cover_bytes).file_name("cover.jpg");

                        bot.send_photo(chat_id, photo)
                            .caption(caption)
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .reply_markup(keyboard)
                            .await?;

                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        continue;
                    }
                }

                let text = format!(
                    "📚 *{}*\n👤 *Автор:* {}\n\nНажмите кнопку ниже, чтобы скачать\\.",
                    escape_md(&book.title),
                    escape_md(&book.author)
                );
                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .reply_markup(keyboard)
                    .await?;

                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }
        Err(e) => {
            tracing::error!(error = ?e, "Request failed");
            bot.send_message(chat_id, "❌ Ошибка при обращении к ROPDS\\.")
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
    }
    Ok(())
}

pub async fn handle_callback(
    bot: Bot,
    q: teloxide::types::CallbackQuery,
    state: SharedState,
) -> ResponseResult<()> {
    let requester_id = callback_user_id(&q);
    if !is_allowed(&state, requester_id) {
        tracing::warn!(user_id = requester_id, "Rejected unauthorized callback");
        let _ = bot
            .answer_callback_query(&q.id)
            .text("Доступ запрещен.")
            .await;
        return Ok(());
    }
    if !allow_request(&state, requester_id) {
        let _ = bot
            .answer_callback_query(&q.id)
            .text("Слишком много запросов. Попробуйте позже.")
            .await;
        return Ok(());
    }

    let Some(data) = q.data.clone() else {
        return Ok(());
    };

    if let Some(id_str) = data.strip_prefix("nav:") {
        let _ = bot.answer_callback_query(&q.id).await;

        let Ok(id) = id_str.parse::<u64>() else {
            return Ok(());
        };
        let Some(target) = state.navigation_cache.get(&id).map(|value| value.clone()) else {
            return Ok(());
        };
        if target.owner_id != requester_id {
            return Ok(());
        }
        state.navigation_cache.remove(&id);
        if let Some(msg) = q.message {
            let chat_id = msg.chat().id;
            let msg_id = msg.id();

            if let Some(path) = target.target.strip_prefix('/') {
                let path = format!("/{path}");
                match state.ropds.get_navigation(&path).await {
                    Ok(items) if !items.is_empty() => {
                        let mut rows = Vec::new();
                        for item in items {
                            let navigation_id = state.next_id.fetch_add(1, Ordering::SeqCst);
                            state.navigation_cache.insert(
                                navigation_id,
                                NavigationTarget {
                                    owner_id: requester_id,
                                    target: item
                                        .href
                                        .clone()
                                        .unwrap_or_else(|| format!("search:{}", item.title)),
                                },
                            );
                            rows.push(vec![teloxide::types::InlineKeyboardButton::callback(
                                item.title,
                                format!("nav:{navigation_id}"),
                            )]);
                        }
                        bot.edit_message_text(chat_id, msg_id, "📂 Выберите вариант:")
                            .reply_markup(teloxide::types::InlineKeyboardMarkup::new(rows))
                            .await?;
                    }
                    Ok(_) => {
                        bot.edit_message_text(chat_id, msg_id, "😔 Список пуст\\.")
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(%error, "Navigation failed");
                        bot.edit_message_text(chat_id, msg_id, "❌ Ошибка при получении списка\\.")
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await?;
                    }
                }
                return Ok(());
            }

            let query = target
                .target
                .strip_prefix("search:")
                .unwrap_or(&target.target);
            let _ = bot
                .edit_message_text(
                    chat_id,
                    msg_id,
                    format!("🔍 Ищу: *{}*...", escape_md(query)),
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await;

            let result = state.ropds.search(query).await;
            let _ = send_books_with_covers(&bot, chat_id, requester_id, &state, &result).await;
        }
        return Ok(());
    }

    let Some(id_str) = data.strip_prefix("dl:") else {
        return Ok(());
    };
    let Ok(id) = id_str.parse::<u64>() else {
        return Ok(());
    };

    let Some(entry) = state.download_cache.get(&id).map(|v| v.clone()) else {
        let _ = bot
            .answer_callback_query(&q.id)
            .text("⌛ Ссылка устарела.")
            .await;
        return Ok(());
    };
    if entry.owner_id != requester_id {
        let _ = bot
            .answer_callback_query(&q.id)
            .text("Эта кнопка предназначена для другого пользователя.")
            .await;
        return Ok(());
    }
    state.download_cache.remove(&id);
    let ctx = entry.context;

    let _ = bot.answer_callback_query(&q.id).await;

    if let Some(msg) = q.message {
        let chat_id = msg.chat().id;
        let msg_id = msg.id();
        let _ = bot
            .edit_message_text(chat_id, msg_id, "⏳ Загружаю книгу...")
            .await;

        let _permit = match state.downloads.acquire().await {
            Ok(permit) => permit,
            Err(error) => {
                tracing::error!(%error, "Download semaphore is closed");
                let _ = bot
                    .edit_message_text(chat_id, msg_id, "❌ Загрузка временно недоступна.")
                    .await;
                return Ok(());
            }
        };

        match state.ropds.download_book(&ctx).await {
            Ok((filepath, filename)) => {
                let mut doc_builder = bot
                    .send_document(
                        chat_id,
                        InputFile::file(&filepath).file_name(filename.clone()),
                    )
                    .caption(format!(
                        "📚 *{}*\n👤 {}",
                        escape_md(&ctx.title),
                        escape_md(&ctx.author)
                    ))
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .reply_parameters(ReplyParameters::new(msg_id));

                if let Some(cover_url) = &ctx.cover_url {
                    if let Some(cover_bytes) = state.ropds.download_cover(cover_url).await {
                        if cover_bytes.len() < 200 * 1024 {
                            let thumb = InputFile::memory(cover_bytes).file_name("cover.jpg");
                            doc_builder = doc_builder.thumbnail(thumb);
                        }
                    }
                }

                let result = doc_builder.await;
                let _ = tokio::fs::remove_file(&filepath).await;

                if result.is_err() {
                    let _ = bot
                        .edit_message_text(chat_id, msg_id, "❌ Ошибка при отправке файла.")
                        .await;
                } else {
                    let _ = bot.delete_message(chat_id, msg_id).await;
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if let Some(size) = err_msg.strip_prefix("FILE_TOO_LARGE:") {
                    let mb = size.parse::<u64>().unwrap_or(0) / 1024 / 1024;
                    let _ = bot
                        .edit_message_text(
                            chat_id,
                            msg_id,
                            format!(
                                "⚠️ Файл слишком большой ({mb} МБ)\\.\nСсылка:\n{}",
                                escape_md(&ctx.url)
                            ),
                        )
                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                        .await;
                } else {
                    tracing::error!(error = %e, "Download failed");
                    let _ = bot
                        .edit_message_text(chat_id, msg_id, "❌ Не удалось скачать файл\\.")
                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                        .await;
                }
            }
        }
    }
    Ok(())
}

fn escape_md(s: &str) -> String {
    const SPECIAL: &[char] = &[
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        if SPECIAL.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
