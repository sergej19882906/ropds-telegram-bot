# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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