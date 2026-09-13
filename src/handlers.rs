use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, InputFile, KeyboardButton, KeyboardMarkup,
    MaybeInaccessibleMessage, ParseMode, ReplyParameters,
};
use teloxide::utils::command::BotCommands;

use crate::ropds::{is_opds_href, Book, DownloadContext, NavItem, RopdsClient};

const TELEGRAM_BUTTON_TEXT_LIMIT: usize = 64;
const COVER_CACHE_TTL: Duration = Duration::from_secs(3600);

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
    pub results_cache: DashMap<u64, CachedResults>,
    pub cover_cache: DashMap<String, CachedCover>,
    pub next_id: AtomicU64,
    pub allowed_user_ids: HashSet<u64>,
    pub allow_all_users: bool,
    pub last_activity: DashMap<u64, Instant>,
    pub downloads: Arc<tokio::sync::Semaphore>,
    pub request_cooldown: Duration,
    pub books_per_page: usize,
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

#[derive(Clone)]
pub struct CachedResults {
    pub owner_id: u64,
    pub books: Vec<Book>,
    pub offset: usize,
}

#[derive(Clone)]
pub struct CachedCover {
    pub created: Instant,
    pub bytes: Vec<u8>,
}

fn message_user_id(msg: &Message) -> Option<u64> {
    msg.from.as_ref().map(|user| user.id.0)
}

fn callback_user_id(q: &teloxide::types::CallbackQuery) -> u64 {
    q.from.id.0
}

fn is_allowed(state: &SharedState, user_id: u64) -> bool {
    state.allow_all_users || state.allowed_user_ids.contains(&user_id)
}

fn allow_request(state: &SharedState, user_id: u64) -> bool {
    let now = Instant::now();
    match state.last_activity.entry(user_id) {
        dashmap::mapref::entry::Entry::Occupied(mut entry) => {
            if now.duration_since(*entry.get()) < state.request_cooldown {
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

fn truncate_button_label(title: &str) -> String {
    if title.chars().count() <= TELEGRAM_BUTTON_TEXT_LIMIT {
        return title.to_string();
    }
    let mut label: String = title.chars().take(TELEGRAM_BUTTON_TEXT_LIMIT - 1).collect();
    label.push('…');
    label
}

fn searching_message(query: &str) -> String {
    format!("🔍 Ищу: *{}*\\.\\.\\.", escape_md(query))
}

fn cache_nav_rows(
    state: &SharedState,
    owner_id: u64,
    items: impl IntoIterator<Item = NavItem>,
) -> Vec<Vec<InlineKeyboardButton>> {
    items
        .into_iter()
        .map(|item| vec![nav_or_download_button(state, owner_id, item)])
        .collect()
}

fn nav_or_download_button(
    state: &SharedState,
    owner_id: u64,
    item: NavItem,
) -> InlineKeyboardButton {
    let label = truncate_button_label(&item.title);
    if let Some(context) = item.download {
        let id = state.next_id.fetch_add(1, Ordering::SeqCst);
        state
            .download_cache
            .insert(id, CachedDownload { owner_id, context });
        return InlineKeyboardButton::callback(label, format!("dl:{id}"));
    }

    let navigation_id = state.next_id.fetch_add(1, Ordering::SeqCst);
    state.navigation_cache.insert(
        navigation_id,
        NavigationTarget {
            owner_id,
            target: item
                .href
                .unwrap_or_else(|| format!("search:{}", item.title)),
        },
    );
    InlineKeyboardButton::callback(label, format!("nav:{navigation_id}"))
}

fn callback_message_is_media(msg: &MaybeInaccessibleMessage) -> bool {
    msg.regular_message().is_some_and(|message| {
        message.photo().is_some()
            || message.video().is_some()
            || message.document().is_some()
            || message.animation().is_some()
    })
}

async fn edit_callback_message(
    bot: &Bot,
    msg: &MaybeInaccessibleMessage,
    text: impl Into<String>,
    parse_mode: Option<ParseMode>,
) -> Result<(), teloxide::RequestError> {
    let chat_id = msg.chat().id;
    let msg_id = msg.id();
    let text = text.into();
    if callback_message_is_media(msg) {
        let mut request = bot.edit_message_caption(chat_id, msg_id).caption(text);
        if let Some(mode) = parse_mode {
            request = request.parse_mode(mode);
        }
        request.await.map(|_| ())
    } else {
        let mut request = bot.edit_message_text(chat_id, msg_id, text);
        if let Some(mode) = parse_mode {
            request = request.parse_mode(mode);
        }
        request.await.map(|_| ())
    }
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
                .parse_mode(ParseMode::MarkdownV2)
                .reply_markup(main_menu())
                .await?;
        }
        Cmd::Search(query) => {
            if query.trim().is_empty() {
                bot.send_message(msg.chat.id, "⚠️ Укажите запрос\\. Пример: `/search Дюна`")
                    .parse_mode(ParseMode::MarkdownV2)
                    .await?;
                return Ok(());
            }
            let result = state.ropds.search(&query).await;
            send_book_results(&bot, msg.chat.id, user_id, &state, result).await?;
        }
        Cmd::Recent => {
            let _ = bot
                .send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
                .await;
            let result = state.ropds.get_recent_books().await;
            send_book_results(&bot, msg.chat.id, user_id, &state, result).await?;
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
                        .parse_mode(ParseMode::MarkdownV2)
                        .await?;
                }
                Ok(items) => {
                    let markup = InlineKeyboardMarkup::new(cache_nav_rows(&state, user_id, items));
                    let title = if matches!(cmd, Cmd::Authors) {
                        "Авторы"
                    } else {
                        "Жанры"
                    };

                    bot.send_message(
                        msg.chat.id,
                        format!("📂 *{}* \\(выберите для поиска\\):", title),
                    )
                    .parse_mode(ParseMode::MarkdownV2)
                    .reply_markup(markup)
                    .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Navigation failed");
                    bot.send_message(msg.chat.id, "❌ Ошибка при получении списка\\.")
                        .parse_mode(ParseMode::MarkdownV2)
                        .await?;
                }
            }
        }
    }
    Ok(())
}

async fn send_book_results(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    owner_id: u64,
    state: &SharedState,
    result: anyhow::Result<Vec<Book>>,
) -> ResponseResult<()> {
    match result {
        Ok(books) if books.is_empty() => {
            bot.send_message(chat_id, "😔 Ничего не найдено\\.")
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
        }
        Ok(books) => {
            send_book_page(bot, chat_id, owner_id, state, books, 0).await?;
        }
        Err(e) => {
            tracing::error!(error = ?e, "Request failed");
            bot.send_message(chat_id, "❌ Ошибка при обращении к ROPDS\\.")
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
        }
    }
    Ok(())
}

async fn send_book_page(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    owner_id: u64,
    state: &SharedState,
    books: Vec<Book>,
    offset: usize,
) -> ResponseResult<()> {
    let page_size = state.books_per_page;
    let total = books.len();
    let page = books.iter().skip(offset).take(page_size);
    for book in page {
        send_book_card(bot, chat_id, owner_id, state, book).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let next_offset = offset.saturating_add(page_size);
    if next_offset < total {
        let remaining = total - next_offset;
        let id = state.next_id.fetch_add(1, Ordering::SeqCst);
        state.results_cache.insert(
            id,
            CachedResults {
                owner_id,
                books,
                offset: next_offset,
            },
        );
        bot.send_message(
            chat_id,
            format!("Показано {next_offset} из {total}. Нажмите, чтобы увидеть ещё."),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback(
                format!("📄 Показать ещё ({remaining})"),
                format!("more:{id}"),
            ),
        ]]))
        .await?;
    }
    Ok(())
}

async fn send_book_card(
    bot: &Bot,
    chat_id: teloxide::types::ChatId,
    owner_id: u64,
    state: &SharedState,
    book: &Book,
) -> ResponseResult<()> {
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

    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "📥 Скачать",
        format!("dl:{id}"),
    )]]);
    let text = format!(
        "📚 *{}*\n👤 *Автор:* {}\n\nНажмите кнопку ниже, чтобы скачать\\.",
        escape_md(&book.title),
        escape_md(&book.author)
    );

    if let Some(cover_url) = &book.cover_url {
        if let Some(cover_bytes) = cached_cover(state, cover_url).await {
            let photo = InputFile::memory(cover_bytes).file_name("cover.jpg");
            bot.send_photo(chat_id, photo)
                .caption(text)
                .parse_mode(ParseMode::MarkdownV2)
                .reply_markup(keyboard)
                .await?;
            return Ok(());
        }
    }

    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(keyboard)
        .await?;
    Ok(())
}

async fn cached_cover(state: &SharedState, url: &str) -> Option<Vec<u8>> {
    if let Some(cached) = state.cover_cache.get(url) {
        if cached.created.elapsed() < COVER_CACHE_TTL {
            return Some(cached.bytes.clone());
        }
    }
    let bytes = state.ropds.download_cover(url).await?;
    state.cover_cache.insert(
        url.to_string(),
        CachedCover {
            created: Instant::now(),
            bytes: bytes.clone(),
        },
    );
    Some(bytes)
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

    if let Some(id_str) = data.strip_prefix("more:") {
        let _ = bot.answer_callback_query(&q.id).await;
        let Ok(id) = id_str.parse::<u64>() else {
            return Ok(());
        };
        let Some(page) = state.results_cache.get(&id).map(|value| value.clone()) else {
            return Ok(());
        };
        if page.owner_id != requester_id {
            return Ok(());
        }
        state.results_cache.remove(&id);
        if let Some(msg) = &q.message {
            let chat_id = msg.chat().id;
            let _ =
                send_book_page(&bot, chat_id, requester_id, &state, page.books, page.offset).await;
        }
        return Ok(());
    }

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

            if is_opds_href(&target.target) {
                match state.ropds.get_navigation(&target.target).await {
                    Ok(items) if !items.is_empty() => {
                        let rows = cache_nav_rows(&state, requester_id, items);
                        bot.edit_message_text(chat_id, msg_id, "📂 Выберите вариант:")
                            .reply_markup(InlineKeyboardMarkup::new(rows))
                            .await?;
                    }
                    Ok(_) => {
                        bot.edit_message_text(chat_id, msg_id, "😔 Список пуст\\.")
                            .parse_mode(ParseMode::MarkdownV2)
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(%error, "Navigation failed");
                        bot.edit_message_text(chat_id, msg_id, "❌ Ошибка при получении списка\\.")
                            .parse_mode(ParseMode::MarkdownV2)
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
                .edit_message_text(chat_id, msg_id, searching_message(query))
                .parse_mode(ParseMode::MarkdownV2)
                .await;

            let result = state.ropds.search(query).await;
            let _ = send_book_results(&bot, chat_id, requester_id, &state, result).await;
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
        let _ = edit_callback_message(&bot, &msg, "⏳ Загружаю книгу...", None).await;

        let _permit = match state.downloads.acquire().await {
            Ok(permit) => permit,
            Err(error) => {
                tracing::error!(%error, "Download semaphore is closed");
                let _ = edit_callback_message(&bot, &msg, "❌ Загрузка временно недоступна.", None)
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
                    .parse_mode(ParseMode::MarkdownV2)
                    .reply_parameters(ReplyParameters::new(msg_id));

                if let Some(cover_url) = &ctx.cover_url {
                    if let Some(cover_bytes) = cached_cover(&state, cover_url).await {
                        if cover_bytes.len() < 200 * 1024 {
                            let thumb = InputFile::memory(cover_bytes).file_name("cover.jpg");
                            doc_builder = doc_builder.thumbnail(thumb);
                        }
                    }
                }

                let result = doc_builder.await;
                let _ = tokio::fs::remove_file(&filepath).await;

                if result.is_err() {
                    let _ =
                        edit_callback_message(&bot, &msg, "❌ Ошибка при отправке файла.", None)
                            .await;
                } else {
                    let _ = bot.delete_message(chat_id, msg_id).await;
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if let Some(size) = err_msg.strip_prefix("FILE_TOO_LARGE:") {
                    let mb = size.parse::<u64>().unwrap_or(0) / 1024 / 1024;
                    let _ = edit_callback_message(
                        &bot,
                        &msg,
                        format!(
                            "⚠️ Файл слишком большой ({mb} МБ)\\.\nСсылка:\n{}",
                            escape_md(&ctx.url)
                        ),
                        Some(ParseMode::MarkdownV2),
                    )
                    .await;
                } else {
                    tracing::error!(error = %e, "Download failed");
                    let _ = edit_callback_message(
                        &bot,
                        &msg,
                        "❌ Не удалось скачать файл\\.",
                        Some(ParseMode::MarkdownV2),
                    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searching_message_escapes_markdown_dots() {
        assert_eq!(searching_message("Дюна"), "🔍 Ищу: *Дюна*\\.\\.\\.");
        assert_eq!(searching_message("A.B"), "🔍 Ищу: *A\\.B*\\.\\.\\.");
    }

    #[test]
    fn truncates_telegram_button_labels() {
        let long = "а".repeat(80);
        let label = truncate_button_label(&long);
        assert_eq!(label.chars().count(), TELEGRAM_BUTTON_TEXT_LIMIT);
        assert!(label.ends_with('…'));
        assert_eq!(truncate_button_label("Дюна"), "Дюна");
    }
}
