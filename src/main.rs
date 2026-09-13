mod config;
mod handlers;
mod ropds;

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use dashmap::DashMap;
use teloxide::prelude::*;

use crate::config::Config;
use crate::handlers::{handle_callback, handle_command, BotState, Cmd, SharedState};
use crate::ropds::RopdsClient;

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

    let ropds = RopdsClient::new(cfg.ropds_url, cfg.ropds_user, cfg.ropds_password)
        .context("Failed to build ROPDS client")?;

    let state: SharedState = Arc::new(BotState {
        ropds,
        download_cache: DashMap::new(),
        navigation_cache: DashMap::new(),
        next_id: AtomicU64::new(1),
        allowed_user_ids: cfg.allowed_user_ids,
        last_activity: DashMap::new(),
        downloads: Arc::new(tokio::sync::Semaphore::new(2)),
    });

    let bot = Bot::new(&cfg.bot_token);

    let cleanup_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            cleanup_state.download_cache.clear();
            cleanup_state.navigation_cache.clear();
            cleanup_state
                .last_activity
                .retain(|_, instant| instant.elapsed() < Duration::from_secs(3600));
            tracing::debug!("Download and navigation caches cleared");
        }
    });

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Cmd>()
                .endpoint(handle_command),
        )
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    tracing::info!("Bot is running. Press Ctrl-C to stop.");

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        // ИСПРАВЛЕНИЕ: метод .enable_ctrlc_handler() удален, так как он удален из teloxide 0.13
        .build()
        .dispatch()
        .await;

    Ok(())
}
