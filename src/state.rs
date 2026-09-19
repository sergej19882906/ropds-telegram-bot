use anyhow::{Context, Result};
use redis::{AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use crate::handlers::{CachedDownload, CachedResults, NavigationTarget};

pub struct StateRepository {
    client: Client,
}

impl StateRepository {
    pub fn new(url: &str) -> Result<Self> {
        let client = Client::open(url).context("Failed to connect to Redis")?;
        Ok(Self { client })
    }

    async fn get_conn(&self) -> Result<redis::aio::Connection> {
        self.client
            .get_async_connection()
            .await
            .context("Failed to get Redis connection")
    }

    pub async fn save<T: Serialize>(&self, key: &str, value: &T, ttl: Duration) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let serialized = serde_json::to_string(value).context("Failed to serialize value")?;
        let _: () = conn.set_ex(key, serialized, ttl.as_secs()).await?;
        Ok(())
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.get_conn().await?;
        let val: Option<String> = conn.get(key).await?;
        match val {
            Some(s) => Ok(Some(
                serde_json::from_str(&s).context("Failed to deserialize value")?,
            )),
            None => Ok(None),
        }
    }

    pub async fn remove(&self, key: &str) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    // Helpers for specific types to keep handlers.rs clean
    pub async fn save_download(&self, id: u64, value: &CachedDownload) -> Result<()> {
        self.save(&format!("dl:{}", id), value, Duration::from_secs(1800))
            .await
    }

    pub async fn get_download(&self, id: u64) -> Result<Option<CachedDownload>> {
        self.get(&format!("dl:{}", id)).await
    }

    pub async fn save_nav(&self, id: u64, value: &NavigationTarget) -> Result<()> {
        self.save(&format!("nav:{}", id), value, Duration::from_secs(1800))
            .await
    }

    pub async fn get_nav(&self, id: u64) -> Result<Option<NavigationTarget>> {
        self.get(&format!("nav:{}", id)).await
    }

    pub async fn save_results(&self, id: u64, value: &CachedResults) -> Result<()> {
        self.save(&format!("res:{}", id), value, Duration::from_secs(1800))
            .await
    }

    pub async fn get_results(&self, id: u64) -> Result<Option<CachedResults>> {
        self.get(&format!("res:{}", id)).await
    }
}
