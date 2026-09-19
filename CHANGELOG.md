# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-09-19

### Added
- State management via Redis for persistence across restarts and scalability
- Individual TTL for cached results, navigation and downloads (30 minutes)
- Search query validation (min length and alphanumeric check)
- Detailed error messages for users (timeouts, connection issues, server errors)
- Configurable timeouts for books, feeds, covers and downloads in `.env`
- Updated Docker Compose with Redis service and healthchecks

### Changed
- Architecture shifted to stateless using Redis `StateRepository`
- Updated documentation and `.env.example` to reflect new requirements

### Fixed
- Minor bug fixes and stability improvements

## [0.1.7] - 2026-09-13

### Added
- Configurable size, concurrency, cooldown, and pagination limits
- Optional `ALLOW_ALL_USERS` switch and startup warning when the bot is public
- Cover cache, “show more” pagination, and Telegram `getMe` readiness file
- Graceful Ctrl-C shutdown and cleanup of leftover download temp files

### Changed
- Book and cover downloads use async streaming instead of blocking HTTP
- OPDS feed bodies are size-limited; publications without acquisition links are skipped
- Release workflow accepts prerelease tags such as `v1.2.3-rc.1`
- MSRV CI job now runs tests

### Fixed
- Safe Unicode filename truncation for long Cyrillic titles
- Author/genre navigation from grouped OPDS feeds and absolute hrefs
- Download status updates on cover photos and MarkdownV2 search status text
- Temporary book files are removed on download errors
- HTTP error bodies are truncated in logs

## [0.1.6] - 2026-09-11

### Fixed
- Request the paginated OPDS recent books feed
- Allow slow OPDS book feeds up to 180 seconds
- Include full request error details in logs

## [0.1.5] - 2026-09-11

### Added
- Persistent Telegram reply menu with search, recent books, authors, genres, and start actions
- Windows and Linux binary deployment documentation
- Docker container startup documentation

### Fixed
- Author and genre navigation using cached callback identifiers and OPDS navigation links
- MarkdownV2 escaping in author and genre menus
- Synology-compatible Docker healthcheck
- UTF-8 BOM in `Cargo.toml`

## [0.1.4] - 2026-09-10

### Fixed
- Resolve OPDS acquisition and cover URLs with URL joining to avoid duplicate slashes and download 404 errors

## [0.1.2] - 2026-09-10

### Added
- ARM64 release targets for Linux and Windows
- GitHub Container Registry publishing workflow
- Cross-platform release build documentation

### Changed
- Ignore local distribution archives under `dist/`

## [0.1.1] - 2026-09-10

### Added
- Initial release
- Search books via OPDS 2.0 (`/opds/v2/search`)
- Download books directly to Telegram chat (up to 50 MB)
- Direct link fallback for large files
- HTTP Basic Auth support
- Docker support with multi-stage build
- GitHub Actions CI/CD with multi-platform releases
- VS Code configuration
- English and Russian documentation