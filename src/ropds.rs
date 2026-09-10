use anyhow::{Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_BOOK_SIZE: u64 = 50 * 1024 * 1024;
const MAX_COVER_SIZE: u64 = 5 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct Book {
    pub title: String,
    pub author: String,
    pub url: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadContext {
    pub url: String,
    pub title: String,
    pub author: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NavItem {
    pub title: String,
}

pub struct RopdsClient {
    http: Client,
    base_url: Url,
    auth_header: Option<String>,
}

impl RopdsClient {
    pub fn new(base_url: String, user: Option<String>, password: Option<String>) -> Result<Self> {
        let base_url = Url::parse(&base_url).context("Invalid ROPDS URL")?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host().is_none() {
            anyhow::bail!("ROPDS URL must use http or https and include a host");
        }

        let mut builder = Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("ropds-telegram-bot/0.1");

        let auth_header = if let (Some(u), Some(p)) = (user, password) {
            let creds = format!("{u}:{p}");
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                creds.as_bytes(),
            );
            let header_val = format!("Basic {encoded}");

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::AUTHORIZATION, header_val.parse()?);
            builder = builder.default_headers(headers);

            Some(header_val)
        } else {
            None
        };

        Ok(Self {
            http: builder.build()?,
            base_url,
            auth_header,
        })
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Book>> {
        let path = format!("/opds/v2/search/{}/?lang=ru", urlencoding::encode(query));
        let url = self.base_url.join(&path)?;
        self.fetch_books(url).await
    }

    pub async fn get_recent_books(&self) -> Result<Vec<Book>> {
        let url = self.base_url.join("/opds/v2/recent/?lang=ru")?;
        self.fetch_books(url).await
    }

    pub async fn get_navigation(&self, path: &str) -> Result<Vec<NavItem>> {
        let url = self.base_url.join(path)?;
        tracing::info!(url = %url, "Fetching navigation");

        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/opds+json")
            .send()
            .await
            .context("Failed to send navigation request")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Navigation request failed with status {}",
                response.status()
            );
        }

        let body = response
            .text()
            .await
            .context("Failed to read response body")?;
        let feed: Opds2Feed = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, "Serde parsing failed");
            anyhow::anyhow!("Failed to parse OPDS 2.0 JSON response: {}", e)
        })?;

        // Берем первые 20 элементов, чтобы не превысить лимиты Telegram
        Ok(feed
            .navigation
            .into_iter()
            .take(20)
            .map(|n| NavItem { title: n.title })
            .collect())
    }

    async fn fetch_books(&self, url: Url) -> Result<Vec<Book>> {
        tracing::info!(url = %url, "OPDS 2.0 books request");

        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/opds+json")
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "ROPDS error");
            anyhow::bail!("ROPDS returned status {status}");
        }

        let body = response
            .text()
            .await
            .context("Failed to read response body")?;
        let feed: Opds2Feed = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, "Serde parsing failed");
            anyhow::anyhow!("Failed to parse OPDS 2.0 JSON response: {}", e)
        })?;

        Ok(feed.into_books(&self.base_url))
    }

    pub async fn download_book(&self, ctx: &DownloadContext) -> Result<(PathBuf, String)> {
        let url = Url::parse(&ctx.url).context("Invalid book URL")?;
        ensure_allowed_url(&url, &self.base_url)?;
        let title = ctx.title.clone();
        let author = ctx.author.clone();
        let auth_header = self.auth_header.clone();
        let base_url = self.base_url.clone();

        let result = tokio::task::spawn_blocking(move || {
            tracing::info!(url = %url, "Starting book download in blocking thread");

            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(120))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| anyhow::anyhow!("Client build error: {}", e))?;
            let mut current_url = url;
            let mut redirects = 0;
            let response = loop {
                if redirects > 5 {
                    anyhow::bail!("Too many redirects while downloading book");
                }
                let mut request = client.get(current_url.clone());
                if let Some(header) = &auth_header {
                    request = request.header(reqwest::header::AUTHORIZATION, header);
                }
                let response = request
                    .send()
                    .map_err(|e| anyhow::anyhow!("Request error: {}", e))?;
                if !response.status().is_redirection() {
                    break response;
                }
                redirects += 1;
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .context("Redirect response did not include a Location header")?;
                current_url = current_url.join(
                    location
                        .to_str()
                        .context("Redirect Location is not valid UTF-8")?,
                )?;
                ensure_allowed_url(&current_url, &base_url)?;
            };

            if !response.status().is_success() {
                anyhow::bail!("Download failed with status {}", response.status());
            }

            let content_length = response.content_length().unwrap_or(0);
            if content_length > MAX_BOOK_SIZE {
                anyhow::bail!("FILE_TOO_LARGE:{content_length}");
            }

            let safe_title = sanitize_filename(&title);
            let safe_author = sanitize_filename(&author);
            let extension = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(extension_from_content_type)
                .unwrap_or("fb2");
            let filename = format!("{} - {}.{extension}", safe_title, safe_author);

            let download_dir = std::env::current_dir()?.join("downloads");
            std::fs::create_dir_all(&download_dir)?;
            let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let filepath = download_dir.join(format!(
                ".{}-{}-{filename}",
                std::process::id(),
                unique_suffix
            ));

            tracing::info!(path = ?filepath, "Saving book to disk");
            let mut file = std::fs::File::create(&filepath)?;
            let bytes_written = std::io::copy(
                &mut response.take(MAX_BOOK_SIZE.saturating_add(1)),
                &mut file,
            )?;
            if bytes_written > MAX_BOOK_SIZE {
                std::fs::remove_file(&filepath)?;
                anyhow::bail!("FILE_TOO_LARGE:{}", bytes_written);
            }

            Ok::<(PathBuf, String), anyhow::Error>((filepath, filename))
        })
        .await
        .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

        Ok(result)
    }

    pub async fn download_cover(&self, url: &str) -> Option<Vec<u8>> {
        let url = Url::parse(url).ok()?;
        if ensure_allowed_url(&url, &self.base_url).is_err() {
            tracing::warn!(url = %url, "Rejected cover URL outside ROPDS origin");
            return None;
        }
        let auth_header = self.auth_header.clone();
        let base_url = self.base_url.clone();

        let result = tokio::task::spawn_blocking(move || {
            let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(15));
            let client = client
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .ok()?;
            let mut current_url = url;
            let mut redirects = 0;
            let response = loop {
                if redirects > 5 {
                    return None;
                }
                let mut request = client.get(current_url.clone());
                if let Some(header) = &auth_header {
                    request = request.header(reqwest::header::AUTHORIZATION, header);
                }
                let response = request.send().ok()?;
                if !response.status().is_redirection() {
                    break response;
                }
                redirects += 1;
                let location = response.headers().get(reqwest::header::LOCATION)?;
                current_url = current_url.join(location.to_str().ok()?).ok()?;
                ensure_allowed_url(&current_url, &base_url).ok()?;
            };

            if !response.status().is_success() {
                return None;
            }

            if response.content_length().unwrap_or(0) > MAX_COVER_SIZE {
                return None;
            }
            let mut body = Vec::new();
            response
                .take(MAX_COVER_SIZE.saturating_add(1))
                .read_to_end(&mut body)
                .ok()?;
            if body.len() as u64 > MAX_COVER_SIZE {
                return None;
            }
            Some(body)
        })
        .await
        .ok()
        .flatten();

        result
    }
}

fn ensure_allowed_url(url: &Url, base_url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("URL scheme is not allowed");
    }
    if url.host_str() != base_url.host_str()
        || url.port_or_known_default() != base_url.port_or_known_default()
    {
        anyhow::bail!("URL host is outside the configured ROPDS origin");
    }
    if base_url.scheme() == "https" && url.scheme() != "https" {
        anyhow::bail!("HTTPS to HTTP downgrade is not allowed");
    }
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let cleaned = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    if cleaned.len() > 100 {
        format!("{}...", &cleaned[..97])
    } else {
        cleaned
    }
}

fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    let mime = content_type.split(';').next()?.trim().to_ascii_lowercase();
    match mime.as_str() {
        "application/pdf" => Some("pdf"),
        "application/epub+zip" => Some("epub"),
        "application/x-mobipocket-ebook" => Some("mobi"),
        "application/x-fictionbook+xml" | "text/xml" | "application/xml" => Some("fb2"),
        _ => None,
    }
}

// ============================================================================
// OPDS 2.0 JSON структуры
// ============================================================================

#[derive(Debug, Deserialize)]
struct Opds2Feed {
    #[serde(default)]
    publications: Vec<Opds2Publication>,
    #[serde(default)]
    groups: Vec<Opds2Group>,
    #[serde(default)]
    navigation: Vec<Opds2Nav>,
}

#[derive(Debug, Deserialize)]
struct Opds2Group {
    #[serde(default)]
    publications: Vec<Opds2Publication>,
}

#[derive(Debug, Deserialize)]
struct Opds2Publication {
    metadata: Opds2Metadata,
    #[serde(default)]
    links: Vec<Opds2Link>,
    #[serde(default)]
    images: Vec<Opds2Image>,
}

#[derive(Debug, Deserialize)]
struct Opds2Metadata {
    #[serde(default)]
    title: String,
    #[serde(default)]
    author: Value,
    #[serde(default)]
    authors: Value,
}

#[derive(Debug, Deserialize)]
struct Opds2Link {
    #[serde(default)]
    href: String,
    #[serde(default)]
    rel: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Opds2Image {
    #[serde(default)]
    href: String,
    #[allow(dead_code)]
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Opds2Nav {
    #[serde(default)]
    title: String,
}

impl Opds2Feed {
    fn into_books(self, base_url: &Url) -> Vec<Book> {
        let mut pubs = self.publications;
        for group in self.groups {
            pubs.extend(group.publications);
        }
        pubs.into_iter().map(|p| p.into_book(base_url)).collect()
    }
}

impl Opds2Publication {
    fn into_book(self, base_url: &Url) -> Book {
        let author = extract_author_name(&self.metadata.author)
            .or_else(|| extract_author_name(&self.metadata.authors))
            .unwrap_or_else(|| "Неизвестный автор".to_string());

        let relative_url = self
            .links
            .iter()
            .find(|link| {
                let rel = link.rel.as_deref().unwrap_or("");
                rel == "http://opds-spec.org/acquisition"
                    || rel == "acquisition"
                    || rel == "http://opds-spec.org/acquisition/open-access"
            })
            .map(|link| link.href.clone())
            .unwrap_or_default();

        let url = base_url
            .join(&relative_url)
            .map(|url| url.to_string())
            .unwrap_or(relative_url);

        let cover_url = self.images.first().map(|img| {
            base_url
                .join(&img.href)
                .map(|url| url.to_string())
                .unwrap_or_else(|_| img.href.clone())
        });

        Book {
            title: self.metadata.title,
            author,
            url,
            cover_url,
        }
    }
}

fn extract_author_name(value: &Value) -> Option<String> {
    if let Some(name) = value.as_str() {
        return Some(name.to_string());
    }
    if let Some(obj) = value.as_object() {
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            return Some(name.to_string());
        }
    }
    if let Some(arr) = value.as_array() {
        if let Some(first) = arr.first() {
            if let Some(name) = first.get("name").and_then(|v| v.as_str()) {
                return Some(name.to_string());
            }
            if let Some(name) = first.as_str() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_cross_origin_urls() {
        let base = Url::parse("https://library.example.test:8443").unwrap();
        let external = Url::parse("https://attacker.example.test:8443/book").unwrap();
        assert!(ensure_allowed_url(&external, &base).is_err());
    }

    #[test]
    fn rejects_https_downgrade() {
        let base = Url::parse("https://library.example.test").unwrap();
        let insecure = Url::parse("http://library.example.test/book").unwrap();
        assert!(ensure_allowed_url(&insecure, &base).is_err());
    }

    #[test]
    fn detects_supported_extensions() {
        assert_eq!(extension_from_content_type("application/pdf"), Some("pdf"));
        assert_eq!(
            extension_from_content_type("application/epub+zip; charset=binary"),
            Some("epub")
        );
        assert_eq!(
            extension_from_content_type("application/octet-stream"),
            None
        );
    }

    #[test]
    fn sanitizes_filenames() {
        assert_eq!(sanitize_filename("a/b:c"), "a_b_c");
    }

    #[test]
    fn resolves_acquisition_links_without_duplicate_slashes() {
        let base = Url::parse("http://library.example.test:8081").unwrap();
        let publication = Opds2Publication {
            metadata: Opds2Metadata {
                title: "Book".to_string(),
                author: Value::String("Author".to_string()),
                authors: Value::Null,
            },
            links: vec![Opds2Link {
                href: "/opds/download/123/0/".to_string(),
                rel: Some("acquisition".to_string()),
                r#type: None,
            }],
            images: Vec::new(),
        };

        let book = publication.into_book(&base);

        assert_eq!(
            book.url,
            "http://library.example.test:8081/opds/download/123/0/"
        );
    }
}
