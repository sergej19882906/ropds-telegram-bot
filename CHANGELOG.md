# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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