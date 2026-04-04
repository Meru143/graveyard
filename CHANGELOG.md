# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.2] - 2026-04-04

### Fixed

- Disabled `actions/setup-node@v6` package-manager caching in the release workflow so npm publishing and npm install smoke tests no longer require a lockfile.
- Re-cut the release after `0.1.1` partially published crates.io and PyPI but failed before npm publication.

## [0.1.1] - 2026-04-04

### Changed

- Hardened GitHub Actions around a stable summary `CI` check so branch protection can require a single status instead of the full matrix.
- Upgraded workflow actions toward Node 24 compatibility and made coverage uploads resilient to transient Codecov failures.
- Added post-publish release smoke tests for `cargo install graveyard`, `pip install graveyard`, and `npm install -g graveyard-cli`.

## [0.1.0] - 2026-04-03

### Added

- Initial `graveyard` CLI with `scan`, `baseline`, `languages`, and `completions` commands.
- Multi-language symbol extraction for Python, JavaScript, TypeScript, Go, and Rust using tree-sitter grammars compiled into the binary.
- Manifest-aware repository walking with `ignore`-crate `.gitignore` support, binary-file skipping, and minified-JS filtering.
- Unified reference graph construction with dead candidate detection, strongly connected component analysis for dead cycles, and test-only caller tagging.
- Git-history scoring via `git2`, including deadness age, recent churn, static-only fallback for shallow or missing repositories, and HEAD-aware caching.
- Confidence scoring built from age, reference count, scope, and churn factors, with support for `--min-age`, `--min-confidence`, `--top`, and exported symbol filtering.
- Output renderers for terminal table, JSON, SARIF 2.1.0, and CSV formats.
- Baseline save and diff workflows for ratcheting new dead code in CI.
- Distribution setup for `pip`, `npm`, `cargo`, and Homebrew-backed release artifacts through GoReleaser and maturin.
- GitHub Actions CI for formatting, linting, tests, release builds, security audit, dependency policy checks, and coverage upload.
- Project documentation, contributor guidance, issue templates, pull request template, and repository security policy.
