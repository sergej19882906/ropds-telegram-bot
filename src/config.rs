use std::collections::HashSet;
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_MAX_BOOK_MB: u64 = 50;
const DEFAULT_MAX_COVER_MB: u64 = 5;
const DEFAULT_MAX_FEED_MB: u64 = 10;
const DEFAULT_CONCURRENT_DOWNLOADS: usize = 2;
const DEFAULT_COOLDOWN_SECS: u64 = 2;
const DEFAULT_BOOKS_PER_PAGE: usize = 5;
const DEFAULT_BOOKS_TIMEOUT_SECS: u64 = 180;
const DEFAULT_FEED_TIMEOUT_SECS: u64 = 60;
const DEFAULT_COVER_TIMEOUT_SECS: u64 = 15;
const DEFAULT_DOWNLOAD_TIMEOUT_SECS: u64 = 120;

#[derive(Clone)]
pub struct Config {
    pub bot_token: String,
    pub ropds_url: String,
    pub ropds_user: Option<String>,
    pub ropds_password: Option<String>,
    pub allowed_user_ids: HashSet<u64>,
    pub allow_all_users: bool,
    pub max_book_size: u64,
    pub max_cover_size: u64,
    pub max_feed_size: u64,
    pub max_concurrent_downloads: usize,
    pub request_cooldown: Duration,
    pub books_per_page: usize,
    pub books_timeout: Duration,
    pub feed_timeout: Duration,
    pub cover_timeout: Duration,
    pub download_timeout: Duration,
    pub redis_url: String,
    pub ready_file: Option<PathBuf>,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("ropds_url", &self.ropds_url)
            .field("ropds_user", &self.ropds_user)
            .field(
                "ropds_password",
                &self.ropds_password.as_ref().map(|_| "***"),
            )
            .field("allowed_user_ids", &self.allowed_user_ids.len())
            .field("allow_all_users", &self.allow_all_users)
            .field("max_book_size", &self.max_book_size)
            .field("max_cover_size", &self.max_cover_size)
            .field("max_feed_size", &self.max_feed_size)
            .field("max_concurrent_downloads", &self.max_concurrent_downloads)
            .field("request_cooldown", &self.request_cooldown)
            .field("books_per_page", &self.books_per_page)
            .field("books_timeout", &self.books_timeout)
            .field("feed_timeout", &self.feed_timeout)
            .field("cover_timeout", &self.cover_timeout)
            .field("download_timeout", &self.download_timeout)
            .field("redis_url", &self.redis_url)
            .field("ready_file", &self.ready_file)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let bot_token =
            env::var("BOT_TOKEN").map_err(|_| anyhow::anyhow!("BOT_TOKEN must be set in .env"))?;

        let ropds_url = env::var("ROPDS_URL")
            .unwrap_or_else(|_| "http://localhost:8081".into())
            .trim_end_matches('/')
            .to_string();

        let ropds_user = env::var("ROPDS_USER").ok().filter(|s| !s.is_empty());
        let ropds_password = env::var("ROPDS_PASSWORD").ok().filter(|s| !s.is_empty());
        let allowed_user_ids = parse_allowlist(&env::var("ALLOWED_USER_IDS").unwrap_or_default())?;
        let allow_all_users = resolve_allow_all(&allowed_user_ids)?;

        let max_book_size = mb_to_bytes(env_u64("MAX_BOOK_SIZE_MB", DEFAULT_MAX_BOOK_MB)?);
        let max_cover_size = mb_to_bytes(env_u64("MAX_COVER_SIZE_MB", DEFAULT_MAX_COVER_MB)?);
        let max_feed_size = mb_to_bytes(env_u64("MAX_FEED_SIZE_MB", DEFAULT_MAX_FEED_MB)?);
        let max_concurrent_downloads =
            env_usize("MAX_CONCURRENT_DOWNLOADS", DEFAULT_CONCURRENT_DOWNLOADS)?.max(1);
        let request_cooldown =
            Duration::from_secs(env_u64("REQUEST_COOLDOWN_SECS", DEFAULT_COOLDOWN_SECS)?);
        let books_per_page = env_usize("BOOKS_PER_PAGE", DEFAULT_BOOKS_PER_PAGE)?.max(1);
        let books_timeout =
            Duration::from_secs(env_u64("BOOKS_TIMEOUT_SECS", DEFAULT_BOOKS_TIMEOUT_SECS)?);
        let feed_timeout =
            Duration::from_secs(env_u64("FEED_TIMEOUT_SECS", DEFAULT_FEED_TIMEOUT_SECS)?);
        let cover_timeout =
            Duration::from_secs(env_u64("COVER_TIMEOUT_SECS", DEFAULT_COVER_TIMEOUT_SECS)?);
        let download_timeout = Duration::from_secs(env_u64(
            "DOWNLOAD_TIMEOUT_SECS",
            DEFAULT_DOWNLOAD_TIMEOUT_SECS,
        )?);
        let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
        let ready_file = env::var("BOT_READY_FILE")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        if ropds_url.starts_with("http://") {
            tracing::warn!(
                ropds_url = %ropds_url,
                "ROPDS_URL uses HTTP; prefer HTTPS if the library is reachable over the network"
            );
        }
        if allow_all_users {
            tracing::warn!(
                "The bot accepts all Telegram users. Set ALLOWED_USER_IDS or ALLOW_ALL_USERS=false for production."
            );
        }

        Ok(Self {
            bot_token,
            ropds_url,
            ropds_user,
            ropds_password,
            allowed_user_ids,
            allow_all_users,
            max_book_size,
            max_cover_size,
            max_feed_size,
            max_concurrent_downloads,
            request_cooldown,
            books_per_page,
            books_timeout,
            feed_timeout,
            cover_timeout,
            download_timeout,
            redis_url,
            ready_file,
        })
    }
}

fn mb_to_bytes(mb: u64) -> u64 {
    mb.saturating_mul(1024 * 1024)
}

fn env_u64(name: &str, default: u64) -> anyhow::Result<u64> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("{name} must be a positive integer")),
        _ => Ok(default),
    }
}

fn env_usize(name: &str, default: usize) -> anyhow::Result<usize> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("{name} must be a positive integer")),
        _ => Ok(default),
    }
}

fn parse_allowlist(raw: &str) -> anyhow::Result<HashSet<u64>> {
    raw.split(',')
        .filter(|id| !id.trim().is_empty())
        .map(|id| {
            id.trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("ALLOWED_USER_IDS contains an invalid user ID"))
        })
        .collect()
}

fn resolve_allow_all(allowed_user_ids: &HashSet<u64>) -> anyhow::Result<bool> {
    match env::var("ALLOW_ALL_USERS") {
        Ok(value) if value.trim().is_empty() => Ok(allowed_user_ids.is_empty()),
        Ok(value) => {
            let allow_all = parse_bool_flag(&value)?;
            if !allow_all && allowed_user_ids.is_empty() {
                anyhow::bail!("ALLOWED_USER_IDS must be set when ALLOW_ALL_USERS=false");
            }
            Ok(allow_all)
        }
        Err(_) => Ok(allowed_user_ids.is_empty()),
    }
}

fn parse_bool_flag(raw: &str) -> anyhow::Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => anyhow::bail!("ALLOW_ALL_USERS must be true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_allowlist() {
        let ids = parse_allowlist("1, 2,3").unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&2));
        assert!(parse_allowlist("").unwrap().is_empty());
        assert!(parse_allowlist("abc").is_err());
    }

    #[test]
    fn parses_bool_flags() {
        assert!(parse_bool_flag("true").unwrap());
        assert!(!parse_bool_flag("0").unwrap());
        assert!(parse_bool_flag("maybe").is_err());
    }

    #[test]
    fn converts_megabytes() {
        assert_eq!(mb_to_bytes(50), 50 * 1024 * 1024);
    }

    #[test]
    fn debug_hides_secrets() {
        let cfg = Config {
            bot_token: "secret-token".into(),
            ropds_url: "http://localhost:8081".into(),
            ropds_user: Some("user".into()),
            ropds_password: Some("hunter2".into()),
            allowed_user_ids: HashSet::new(),
            allow_all_users: true,
            max_book_size: 1,
            max_cover_size: 1,
            max_feed_size: 1,
            max_concurrent_downloads: 1,
            request_cooldown: Duration::from_secs(2),
            books_per_page: 5,
            books_timeout: Duration::from_secs(180),
            feed_timeout: Duration::from_secs(60),
            cover_timeout: Duration::from_secs(15),
            download_timeout: Duration::from_secs(120),
            redis_url: "redis://127.0.0.1:6379".into(),
            ready_file: None,
        };
        let debug = format!("{cfg:?}");
        assert!(!debug.contains("hunter2"));
        assert!(!debug.contains("secret-token"));
        assert!(debug.contains("***"));
    }
}
