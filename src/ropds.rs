use anyhow::{Context, Result};
use reqwest::{Client, Response, Url};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;

const OPDS_BOOKS_TIMEOUT: Duration = Duration::from_secs(180);
const OPDS_FEED_TIMEOUT: Duration = Duration::from_secs(60);
const OPDS_COVER_TIMEOUT: Duration = Duration::from_secs(15);
const OPDS_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_FEED_REDIRECTS: u32 = 5;
const MAX_FILENAME_STEM: usize = 100;
const LOG_BODY_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy)]
pub struct ClientLimits {
    pub max_book_size: u64,
    pub max_cover_size: u64,
    pub max_feed_size: u64,
}

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
    limits: ClientLimits,
}

impl RopdsClient {
    pub fn new(
        base_url: String,
        user: Option<String>,
        password: Option<String>,
        limits: ClientLimits,
    ) -> Result<Self> {
        let base_url = Url::parse(&base_url).context("Invalid ROPDS URL")?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host().is_none() {
            anyhow::bail!("ROPDS URL must use http or https and include a host");
        }

        let mut builder = Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("ropds-telegram-bot/0.1");

        if let (Some(u), Some(p)) = (user, password) {
            let creds = format!("{u}:{p}");
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                creds.as_bytes(),
            );
            let header_val = format!("Basic {encoded}");

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::AUTHORIZATION, header_val.parse()?);
            builder = builder.default_headers(headers);
        }

        Ok(Self {
            http: builder.build()?,
            base_url,
            limits,
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
        tracing::info!(url = %url, "OPDS 2.0 feed request");
        let response = self
            .send_following_redirects(url, timeout, "application/opds+json")
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = read_limited_bytes(response, self.limits.max_feed_size)
                .await
                .unwrap_or_default();
            let body = String::from_utf8_lossy(&body);
            tracing::error!(
                status = %status,
                body = %truncate_for_log(&body, LOG_BODY_LIMIT),
                "ROPDS error"
            );
            anyhow::bail!("ROPDS returned status {status}");
        }

        let body = read_limited_bytes(response, self.limits.max_feed_size)
            .await
            .context("Failed to read response body")?;
        let body = String::from_utf8(body).context("OPDS feed is not valid UTF-8")?;
        serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, "Serde parsing failed");
            anyhow::anyhow!("Failed to parse OPDS 2.0 JSON response: {}", e)
        })
    }

    async fn send_following_redirects(
        &self,
        url: Url,
        timeout: Duration,
        accept: &str,
    ) -> Result<Response> {
        let mut current_url = url;
        for _ in 0..=MAX_FEED_REDIRECTS {
            ensure_allowed_url(&current_url, &self.base_url)?;
            let response = self
                .http
                .get(current_url.clone())
                .timeout(timeout)
                .header(reqwest::header::ACCEPT, accept)
                .send()
                .await?;

            if !response.status().is_redirection() {
                return Ok(response);
            }

            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .context("Redirect response did not include a Location header")?
                .to_str()
                .context("Redirect Location is not valid UTF-8")?
                .to_owned();
            current_url = current_url.join(&location)?;
        }
        anyhow::bail!("Too many redirects while fetching the OPDS feed")
    }

    pub async fn download_book(&self, ctx: &DownloadContext) -> Result<(PathBuf, String)> {
        let url = Url::parse(&ctx.url).context("Invalid book URL")?;
        tracing::info!(url = %url, "Starting book download");
        let response = self
            .send_following_redirects(url, OPDS_DOWNLOAD_TIMEOUT, "*/*")
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status {}", response.status());
        }

        let content_length = response.content_length().unwrap_or(0);
        if content_length > self.limits.max_book_size {
            anyhow::bail!("FILE_TOO_LARGE:{content_length}");
        }

        let safe_title = sanitize_filename(&ctx.title);
        let safe_author = sanitize_filename(&ctx.author);
        let extension = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(extension_from_content_type)
            .unwrap_or("fb2");
        let filename = format!("{} - {}.{extension}", safe_title, safe_author);

        let download_dir = downloads_dir()?;
        tokio::fs::create_dir_all(&download_dir).await?;
        let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let filepath = download_dir.join(format!(
            ".{}-{}-{filename}",
            std::process::id(),
            unique_suffix
        ));

        tracing::info!(path = ?filepath, "Saving book to disk");
        let mut guard = TempPath::create(&filepath).await?;
        let bytes_written = write_limited_file(
            guard.file.as_mut().expect("temp file"),
            response,
            self.limits.max_book_size,
        )
        .await?;
        if bytes_written > self.limits.max_book_size {
            anyhow::bail!("FILE_TOO_LARGE:{bytes_written}");
        }
        guard.file.as_mut().expect("temp file").flush().await?;
        guard.keep();
        Ok((filepath, filename))
    }

    pub async fn download_cover(&self, url: &str) -> Option<Vec<u8>> {
        let url = Url::parse(url).ok()?;
        if ensure_allowed_url(&url, &self.base_url).is_err() {
            tracing::warn!(url = %url, "Rejected cover URL outside ROPDS origin");
            return None;
        }
        let response = self
            .send_following_redirects(url, OPDS_COVER_TIMEOUT, "image/*")
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        if response.content_length().unwrap_or(0) > self.limits.max_cover_size {
            return None;
        }
        let body = read_limited_bytes(response, self.limits.max_cover_size)
            .await
            .ok()?;
        if body.len() as u64 > self.limits.max_cover_size {
            return None;
        }
        Some(body)
    }
}

struct TempPath {
    path: PathBuf,
    file: Option<tokio::fs::File>,
    keep: bool,
}

impl TempPath {
    async fn create(path: &Path) -> Result<Self> {
        let file = tokio::fs::File::create(path).await?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
            keep: false,
        })
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        if !self.keep {
            self.file.take();
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn downloads_dir() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("downloads"))
}

pub fn cleanup_downloads_dir() {
    let Ok(dir) = downloads_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn exceeds_limit(current: u64, incoming: usize, max: u64) -> bool {
    current.saturating_add(incoming as u64) > max
}

async fn read_limited_bytes(mut response: Response, max: u64) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if exceeds_limit(body.len() as u64, chunk.len(), max) {
            anyhow::bail!("response body exceeded {max} bytes");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn write_limited_file(
    file: &mut tokio::fs::File,
    mut response: Response,
    max: u64,
) -> Result<u64> {
    let mut written = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        if exceeds_limit(written, chunk.len(), max) {
            written = written.saturating_add(chunk.len() as u64);
            break;
        }
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
    }
    Ok(written)
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
        let mut skipped = 0;
        let books = pubs
            .into_iter()
            .filter_map(|publication| {
                if publication.acquisition_href().is_none() {
                    skipped += 1;
                    return None;
                }
                Some(publication.into_book(base_url))
            })
            .collect();
        if skipped > 0 {
            tracing::warn!(
                skipped,
                "Skipped OPDS publications without an acquisition link"
            );
        }
        books
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
            (is_acquisition_rel(rel) && !link.href.is_empty()).then_some(link.href.as_str())
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

fn is_acquisition_rel(rel: &str) -> bool {
    rel == "acquisition" || rel.contains("opds-spec.org/acquisition")
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
        assert!(sanitize_filename(&"📚".repeat(80)).ends_with("..."));
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

    #[test]
    fn skips_publications_without_acquisition_in_search_results() {
        let base = Url::parse("http://library.example.test:8081").unwrap();
        let feed = Opds2Feed {
            publications: vec![
                Opds2Publication {
                    metadata: Opds2Metadata {
                        title: "Missing".to_string(),
                        author: Value::Null,
                        authors: Value::Null,
                    },
                    links: Vec::new(),
                    images: Vec::new(),
                },
                Opds2Publication {
                    metadata: Opds2Metadata {
                        title: "Book".to_string(),
                        author: Value::String("Author".to_string()),
                        authors: Value::Null,
                    },
                    links: vec![Opds2Link {
                        href: "/opds/download/1/".to_string(),
                        rel: Some("http://opds-spec.org/acquisition/open-access".to_string()),
                        r#type: None,
                    }],
                    images: Vec::new(),
                },
            ],
            groups: Vec::new(),
            navigation: Vec::new(),
        };
        let books = feed.into_books(&base);
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "Book");
    }

    #[test]
    fn extracts_authors_from_string_object_and_array() {
        assert_eq!(
            extract_author_name(&Value::String("Herbert".into())).as_deref(),
            Some("Herbert")
        );
        assert_eq!(
            extract_author_name(&serde_json::json!({"name": "Herbert"})).as_deref(),
            Some("Herbert")
        );
        assert_eq!(
            extract_author_name(&serde_json::json!([{"name": "Herbert"}, {"name": "Other"}]))
                .as_deref(),
            Some("Herbert")
        );
    }

    #[test]
    fn size_limit_detects_overflow() {
        assert!(!exceeds_limit(10, 5, 15));
        assert!(exceeds_limit(10, 6, 15));
    }
}
