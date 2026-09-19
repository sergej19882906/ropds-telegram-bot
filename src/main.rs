mod config;
mod handlers;
mod ropds;
mod state;

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use dashmap::DashMap;
use teloxide::prelude::*;

use crate::config::Config;
use crate::handlers::{handle_callback, handle_command, BotState, Cmd, SharedState};
use crate::ropds::{cleanup_downloads_dir, ClientLimits, RopdsClient};
use crate::state::StateRepository;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(e) = run().await {
        tracing::error!("Application fatal error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cfg = Config::from_env().context("Failed to load config")?;
    tracing::info!(ropds_url = %cfg.ropds_url, "Starting ROPDS Telegram bot");

    cleanup_downloads_dir();

    let ropds = RopdsClient::new(crate::ropds::RopdsConfig {
        base_url: cfg.ropds_url,
        user: cfg.ropds_user,
        password: cfg.ropds_password,
        limits: ClientLimits {
            max_book_size: cfg.max_book_size,
            max_cover_size: cfg.max_cover_size,
            max_feed_size: cfg.max_feed_size,
        },
        books_timeout: cfg.books_timeout,
        feed_timeout: cfg.feed_timeout,
        cover_timeout: cfg.cover_timeout,
        download_timeout: cfg.download_timeout,
    })
    .context("Failed to build ROPDS client")?;

    let state: SharedState = Arc::new(BotState {
        ropds,
        store: StateRepository::new(&cfg.redis_url)
            .context("Failed to create Redis state repository")?,
        cover_cache: DashMap::new(),
        next_id: AtomicU64::new(1),
        allowed_user_ids: cfg.allowed_user_ids,
        allow_all_users: cfg.allow_all_users,
        last_activity: DashMap::new(),
        downloads: Arc::new(tokio::sync::Semaphore::new(cfg.max_concurrent_downloads)),
        request_cooldown: cfg.request_cooldown,
        books_per_page: cfg.books_per_page,
    });

    let bot = Bot::new(&cfg.bot_token);
    let me = bot
        .get_me()
        .await
        .context("Failed to call Telegram getMe")?;
    tracing::info!(username = ?me.username, "Telegram bot authorized");
    if let Some(ready_file) = &cfg.ready_file {
        if let Some(parent) = ready_file.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(ready_file, b"ok")
            .await
            .with_context(|| format!("Failed to write ready file {}", ready_file.display()))?;
    }

    let cleanup_state = Arc::clone(&state);
    let ready_file = cfg.ready_file.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(600)).await;
            cleanup_state
                .cover_cache
                .retain(|_, cached| cached.created.elapsed() < Duration::from_secs(3600));
            cleanup_state
                .last_activity
                .retain(|_, instant| instant.elapsed() < Duration::from_secs(3600));
            if let Some(path) = &ready_file {
                let _ = tokio::fs::write(path, b"ok").await;
            }
            tracing::debug!("Caches cleaned");
        }
    });

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Cmd>()
                .endpoint(handle_command),
        )
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build();
    let shutdown_token = dispatcher.shutdown_token();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("Ctrl-C received, shutting down");
            if let Ok(shutdown) = shutdown_token.shutdown() {
                shutdown.await;
            }
        }
    });

    tracing::info!("Bot is running. Press Ctrl-C to stop.");
    dispatcher.dispatch().await;
    cleanup_downloads_dir();
    Ok(())
}
