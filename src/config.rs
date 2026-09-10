use std::collections::HashSet;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub ropds_url: String,
    pub ropds_user: Option<String>,
    pub ropds_password: Option<String>,
    pub allowed_user_ids: HashSet<u64>,
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
        let allowed_user_ids = env::var("ALLOWED_USER_IDS")
            .unwrap_or_default()
            .split(',')
            .filter(|id| !id.trim().is_empty())
            .map(|id| {
                id.trim()
                    .parse()
                    .map_err(|_| anyhow::anyhow!("ALLOWED_USER_IDS contains an invalid user ID"))
            })
            .collect::<anyhow::Result<HashSet<u64>>>()?;

        Ok(Self {
            bot_token,
            ropds_url,
            ropds_user,
            ropds_password,
            allowed_user_ids,
        })
    }
}
