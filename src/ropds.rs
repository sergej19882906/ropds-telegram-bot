use anyhow::{Context, Result};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_BOOK_SIZE: u64 = 50 * 1024 * 1024;
const MAX_COVER_SIZE: u64 = 5 * 1024 * 1024;
const OPDS_BOOKS_TIMEOUT: Duration = Duration::from_secs(180);
const OPDS_FEED_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FEED_REDIRECTS: u32 = 5;
const MAX_FILENAME_STEM: usize = 100;
const LOG_BODY_LIMIT: usize = 512;

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
    pub href: Option<String>,
    pub download: Option<DownloadContext>,
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
        let url = self.base_url.join("/opds/v2/recent/1/?lang=ru")?;
        self.fetch_books(url).await
    }

    pub async fn get_navigation(&self, href: &str) -> Result<Vec<NavItem>> {
        let url = self.base_url.join(href)?;
        let feed = self.fetch_feed(url, OPDS_FEED_TIMEOUT).await?;
        Ok(feed.into_nav_items(&self.base_url))
    }

    async fn fetch_books(&self, url: Url) -> Result<Vec<Book>> {
        let feed = self.fetch_feed(url, OPDS_BOOKS_TIMEOUT).await?;
        Ok(feed.into_books(&self.base_url))
    }

    /// Клиент собран с `redirect::Policy::none()`, поэтому редиректы
    /// сопровождаются вручную: каждый переход проверяется
    /// `ensure_allowed_url`, чтобы Basic Auth не ушёл на посторонний хост.
    async fn fetch_feed(&self, url: Url, timeout: Duration) -> Result<Opds2Feed> {
        let mut current_url = url;
        for _ in 0..=MAX_FEED_REDIRECTS {
            ensure_allowed_url(&current_url, &self.base_url)?;
            tracing::info!(url = %current_url, "OPDS 2.0 feed request");

            let response = self
                .http
                .get(current_url.clone())
                .timeout(timeout)
                .header(reqwest::header::ACCEPT, "application/opds+json")
                .send()
                .await
                .context("Failed to send request")?;

            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .context("Redirect response did not include a Location header")?
                    .to_str()
                    .context("Redirect Location is not valid UTF-8")?
                    .to_owned();
                current_url = current_url.join(&location)?;
                continue;
            }

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::error!(
                    status = %status,
                    body = %truncate_for_log(&body, LOG_BODY_LIMIT),
                    "ROPDS error"
                );
                anyhow::bail!("ROPDS returned status {status}");
            }

            let body = response
                .text()
                .await
                .context("Failed to read response body")?;
            return serde_json::from_str(&body).map_err(|e| {
                tracing::error!(error = %e, "Serde parsing failed");
                anyhow::anyhow!("Failed to parse OPDS 2.0 JSON response: {}", e)
            });
        }
        anyhow::bail!("Too many redirects while fetching the OPDS feed");
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
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned.bytes().all(|b| b == b'_') {
        return "book".to_string();
    }
    if cleaned.len() <= MAX_FILENAME_STEM {
        return cleaned.to_string();
    }

    // Обрезать нужно по границе символа: названия на кириллице занимают два
    // байта на символ, и срез по фиксированному индексу вызвал бы панику.
    let cut = floor_char_boundary(cleaned, MAX_FILENAME_STEM - "...".len());
    format!("{}...", cut.trim_end())
}

fn floor_char_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn truncate_for_log(text: &str, max_bytes: usize) -> String {
    let cut = floor_char_boundary(text, max_bytes);
    let mut out = cut.to_owned();
    if out.len() < text.len() {
        out.push('…');
    }
    out
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
    #[serde(default)]
    navigation: Vec<Opds2Nav>,
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
    #[serde(default)]
    href: Option<String>,
}

impl Opds2Feed {
    fn into_books(self, base_url: &Url) -> Vec<Book> {
        let mut pubs = self.publications;
        for group in self.groups {
            pubs.extend(group.publications);
        }
        pubs.into_iter().map(|p| p.into_book(base_url)).collect()
    }

    fn into_nav_items(self, base_url: &Url) -> Vec<NavItem> {
        let mut navigation = self.navigation;
        let mut publications = self.publications;
        for group in self.groups {
            navigation.extend(group.navigation);
            publications.extend(group.publications);
        }

        // Telegram ограничивает число кнопок; берём первые 20 пунктов.
        navigation
            .into_iter()
            .map(|item| item.into_nav_item(base_url))
            .chain(
                publications
                    .into_iter()
                    .map(|publication| publication.into_nav_item(base_url)),
            )
            .take(20)
            .collect()
    }
}

impl Opds2Nav {
    fn into_nav_item(self, base_url: &Url) -> NavItem {
        NavItem {
            title: self.title,
            href: self.href.map(|href| resolve_href(base_url, &href)),
            download: None,
        }
    }
}

impl Opds2Publication {
    fn acquisition_href(&self) -> Option<&str> {
        self.links.iter().find_map(|link| {
            let rel = link.rel.as_deref().unwrap_or("");
            let is_acquisition = rel == "http://opds-spec.org/acquisition"
                || rel == "acquisition"
                || rel == "http://opds-spec.org/acquisition/open-access";
            (is_acquisition && !link.href.is_empty()).then_some(link.href.as_str())
        })
    }

    fn into_nav_item(self, base_url: &Url) -> NavItem {
        if self.acquisition_href().is_none() {
            return NavItem {
                title: self.metadata.title,
                href: None,
                download: None,
            };
        }

        let book = self.into_book(base_url);
        NavItem {
            title: book.title.clone(),
            href: None,
            download: Some(DownloadContext {
                url: book.url,
                title: book.title,
                author: book.author,
                cover_url: book.cover_url,
            }),
        }
    }

    fn into_book(self, base_url: &Url) -> Book {
        let author = extract_author_name(&self.metadata.author)
            .or_else(|| extract_author_name(&self.metadata.authors))
            .unwrap_or_else(|| "Неизвестный автор".to_string());

        let relative_url = self.acquisition_href().unwrap_or("").to_string();

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

fn resolve_href(base_url: &Url, href: &str) -> String {
    base_url
        .join(href)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| href.to_string())
}

pub fn is_opds_href(target: &str) -> bool {
    if target.starts_with("search:") || target.is_empty() {
        return false;
    }
    if let Ok(url) = Url::parse(target) {
        return matches!(url.scheme(), "http" | "https");
    }
    target.starts_with('/') || target.contains('/')
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
        assert_eq!(sanitize_filename("  Дюна  "), "Дюна");
        assert_eq!(sanitize_filename("///"), "book");
    }

    #[test]
    fn truncates_long_cyrillic_titles_without_panicking() {
        // Байтовый срез посреди многобайтового символа раньше ронял процесс.
        let titles = [
            "Основания и империя. Часть вторая: Генерал и его замысел о власти",
            "Приключения Незнайки и его друзей в солнечном городе, полное издание",
            "Мастер и Маргарита. Том первый: Чёрная магия и её разоблачение",
            "Гарри Поттер и философский камень. Иллюстрированное издание, глава 1",
        ];
        for title in titles {
            assert!(title.len() > MAX_FILENAME_STEM, "{title} too short");
            let name = sanitize_filename(title);
            assert!(name.ends_with("..."), "{name}");
            assert!(name.len() <= MAX_FILENAME_STEM + 3, "{name}");
            assert!(name.chars().all(|c| c != '/' && c != ':'), "{name}");
        }
    }

    #[test]
    fn floor_char_boundary_never_splits_a_character() {
        let text = "абвгдежзийклмнопрстуфхцчшщъыьэюя";
        for limit in 0..=text.len() + 1 {
            let cut = floor_char_boundary(text, limit);
            assert!(cut.len() <= limit);
            assert!(text.starts_with(cut));
        }
    }

    #[test]
    fn truncates_long_log_bodies() {
        assert_eq!(truncate_for_log("short", LOG_BODY_LIMIT), "short");
        let long = "ы".repeat(LOG_BODY_LIMIT + 10);
        let truncated = truncate_for_log(&long, LOG_BODY_LIMIT);
        assert!(truncated.ends_with('…'));
        assert!(truncated.len() <= LOG_BODY_LIMIT + "…".len());
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

    #[test]
    fn detects_opds_hrefs() {
        assert!(is_opds_href("/opds/v2/authors/1/"));
        assert!(is_opds_href("opds/v2/authors/1/"));
        assert!(is_opds_href(
            "http://library.example.test:8081/opds/v2/authors/1/"
        ));
        assert!(!is_opds_href("search:Дюна"));
        assert!(!is_opds_href("Дюна"));
        assert!(!is_opds_href(""));
    }

    #[test]
    fn navigation_includes_grouped_links_and_downloadable_books() {
        let base = Url::parse("http://library.example.test:8081").unwrap();
        let feed = Opds2Feed {
            publications: Vec::new(),
            navigation: vec![Opds2Nav {
                title: "Top".to_string(),
                href: Some("/opds/v2/authors/1/".to_string()),
            }],
            groups: vec![Opds2Group {
                navigation: vec![Opds2Nav {
                    title: "Grouped".to_string(),
                    href: Some("http://library.example.test:8081/opds/v2/genres/2/".to_string()),
                }],
                publications: vec![Opds2Publication {
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
                }],
            }],
        };

        let items = feed.into_nav_items(&base);
        assert_eq!(items.len(), 3);
        assert_eq!(
            items[0].href.as_deref(),
            Some("http://library.example.test:8081/opds/v2/authors/1/")
        );
        assert_eq!(
            items[1].href.as_deref(),
            Some("http://library.example.test:8081/opds/v2/genres/2/")
        );
        let download = items[2]
            .download
            .as_ref()
            .expect("book should be downloadable");
        assert_eq!(
            download.url,
            "http://library.example.test:8081/opds/download/123/0/"
        );
    }

    #[test]
    fn publications_without_acquisition_are_not_downloads() {
        let base = Url::parse("http://library.example.test:8081").unwrap();
        let publication = Opds2Publication {
            metadata: Opds2Metadata {
                title: "Дюна".to_string(),
                author: Value::Null,
                authors: Value::Null,
            },
            links: Vec::new(),
            images: Vec::new(),
        };

        let item = publication.into_nav_item(&base);
        assert!(item.download.is_none());
        assert!(item.href.is_none());
        assert_eq!(item.title, "Дюна");
    }
}
