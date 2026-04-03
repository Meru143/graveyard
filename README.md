# graveyard

[![CI](https://github.com/you/graveyard/actions/workflows/ci.yml/badge.svg)](https://github.com/you/graveyard/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/graveyard.svg)](https://crates.io/crates/graveyard)
[![PyPI](https://img.shields.io/pypi/v/graveyard.svg)](https://pypi.org/project/graveyard/)
[![npm](https://img.shields.io/npm/v/graveyard.svg)](https://www.npmjs.com/package/graveyard)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`graveyard` scans Python, JavaScript, TypeScript, Go, and Rust repositories for dead code in a single pass, then ranks every finding with a git-history-weighted confidence score. It exists because AI coding agents make cross-language dead code cheaper to create than to notice, and the existing toolchain still forces teams to stitch together language-specific scanners with no shared scoring model.

## What is graveyard?

`graveyard` is a compiled Rust CLI that walks a repository once, extracts symbols with tree-sitter, builds a unified reference graph, folds in git age and churn, then emits a ranked report in table, JSON, CSV, or SARIF form. The front-door workflow is intentionally simple: use `--min-age` when you want "show me code that has been dead for a while" and use `--min-confidence` when you want CI-grade filtering across exported APIs, dead cycles, and fresh code that might still be in flight.

The repository scanner is manifest-aware, so `pyproject.toml`, `package.json`, `go.mod`, and `Cargo.toml` shape language detection automatically. `.gitignore` handling comes from the `ignore` crate, git history comes from `git2`, and the baseline commands let teams ratchet new dead code without having to clean an existing backlog in one change.

## Installation

### pip / pipx

```bash
pip install graveyard
pipx install graveyard
uvx graveyard --version
```

### npm

```bash
npm install -g graveyard
npx graveyard --version
```

### cargo

```bash
cargo install graveyard
graveyard --version
```

### Homebrew

```bash
brew install graveyard
graveyard --version
```

## Quick Start

Run a scan in the current repository:

```bash
graveyard scan
```

Filter for code that has been dead for at least a month:

```bash
graveyard scan --min-age 30d
```

Use CI gating with a stricter score threshold:

```bash
graveyard scan --ci --min-confidence 0.8
```

Sample terminal output:

```text
CONFIDENCE  TAG               AGE         LOCATION                  FQN
0.94        ExportedUnused    1.1 years   src/lib.rs:42             src/lib.rs::legacy::old_api
0.88        Dead              8 months    services/api/foo.py:17    services/api/foo.py::cleanup_task
Found 2 dead symbol(s) — min-confidence 0.8, min-age 30 days
```

## Usage

The default scan targets the current directory and prints a ranked table:

```bash
graveyard scan
graveyard scan ./services/api
graveyard scan --top 25
graveyard scan --format json
graveyard scan --format sarif --output graveyard.sarif
graveyard scan --format csv --output graveyard.csv
```

Time-based filtering is the fastest way to adopt the tool in an existing repository:

```bash
graveyard scan --min-age 7d
graveyard scan --min-age 30d --min-confidence 0.7
graveyard scan --min-age 1y --ignore-exports
```

Repository-specific controls map directly to the implemented flags:

```bash
graveyard scan --exclude "vendor/**" --exclude "generated/**"
graveyard scan --baseline .graveyard-baseline.json
graveyard scan --no-git
graveyard scan --no-cache
graveyard scan --cache-dir ~/.cache/graveyard
graveyard scan --config .graveyard.toml
graveyard scan -v
graveyard scan -vv
```

Baseline management and language detection are first-class commands:

```bash
graveyard baseline save --output .graveyard-baseline.json
graveyard baseline diff --baseline .graveyard-baseline.json
graveyard baseline diff --baseline .graveyard-baseline.json --ci
graveyard languages
graveyard completions bash > ~/.local/share/bash-completion/completions/graveyard
graveyard completions zsh > ~/.zfunc/_graveyard
graveyard completions fish > ~/.config/fish/completions/graveyard.fish
graveyard completions powershell > graveyard.ps1
```

## CI Integration

The ratchet workflow is the cleanest way to add `graveyard` to an existing codebase because it only fails the build when a pull request introduces new dead code relative to a stored baseline.

```yaml
name: Dead Code

on:
  pull_request:
  push:
    branches:
      - main

jobs:
  graveyard:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - name: Install graveyard
        run: pip install graveyard
      - name: Enforce new dead code only
        run: graveyard baseline diff --baseline .graveyard-baseline.json --ci
```

If you do not need a ratchet, replace the final command with `graveyard scan --ci --min-confidence 0.8`. SARIF output is available through `graveyard scan --format sarif --output graveyard.sarif` when you want to upload findings into GitHub code scanning.

## Configuration

`graveyard` resolves settings in this order: CLI flags, `.graveyard.toml`, environment variables, then built-in defaults. The configuration file lives at `.graveyard.toml` by default and supports the full v1 surface.

```toml
[graveyard]
min_confidence = 0.6
min_age = "30d"
fail_on_findings = false
top = 0
format = "table"
output = "graveyard-report.json"
exclude = ["migrations/**", "**/generated/**"]
ignore_exports = false
baseline = ".graveyard-baseline.json"
no_git = false
no_cache = false

[scoring]
age_weight = 0.35
ref_weight = 0.30
scope_weight = 0.20
churn_weight = 0.15
age_max_days = 730
age_min_days = 7

[ignore]
names = ["legacy_*", "TODO_*", "test_*"]
files = ["migrations/**", "**/generated/**", "**/vendor/**"]
decorators = ["@pytest.fixture", "@app.route"]

[languages]
enabled = ["python", "javascript", "typescript", "go", "rust"]

[entry_points]
names = ["main", "__main__", "app", "handler", "create_app"]

[cache]
enabled = true
dir = "~/.cache/graveyard"
```

The `GRAVEYARD_MIN_CONFIDENCE` environment variable can override the default confidence threshold when the config file leaves it unset. `NO_COLOR` and `GRAVEYARD_NO_COLOR` both disable ANSI color output.

## Understanding Scores

`--min-age` is the intended on-ramp because it maps directly to how engineers reason about stale code. If a symbol has had no meaningful touch for thirty days and still has zero reachable callers, it belongs high in the queue even before anyone thinks about the full formula.

`--min-confidence` exposes the full score for CI and team policy work. The score is a weighted sum of four factors: age of deadness, reference count, symbol scope, and recent churn. Local private functions with no callers and no recent history score higher than public APIs or code that changed this week.

```text
confidence =
  0.35 * age_factor(deadness_age_days)
  0.30 * ref_factor(in_degree)
  0.20 * scope_factor(symbol)
  0.15 * churn_factor(commits_90d)
```

Those weights are configurable in `[scoring]`, but they must still sum to `1.0`. Use `--ignore-exports` when a repository has many intentionally public APIs and you only want truly unreachable internals.

## Language Support

| Language | Status | Notes |
| --- | --- | --- |
| Python | Yes | Functions, classes, `__all__`, decorator-aware extraction |
| JavaScript | Yes | Functions, arrow functions, exports, `export * from` |
| TypeScript | Yes | JS support plus interfaces, type aliases, TSX parsing |
| Go | Yes | Functions, methods, exported identifier detection |
| Rust | Yes | Functions, methods, structs, enums, `pub` visibility, test attributes |

## vs. Other Tools

| Tool | Language Scope | Git History Scoring | Baseline Ratchet | Install Surface |
| --- | --- | --- | --- | --- |
| `graveyard` | Python, JS, TS, Go, Rust | Yes | Yes | `pip`, `npm`, `cargo`, `brew` |
| `vulture` | Python only | No | No | Python |
| `knip` | JS/TS only | No | No | npm |
| `deadcode` | Go only | No | No | Go toolchain |
| `cargo-machete` | Rust dependency analysis | No | No | cargo |

`graveyard` is not trying to replace dependency-pruning tools such as `cargo-machete`. It sits at the source-code layer, where teams need one ranked list across a polyglot repository instead of separate outputs from five ecosystems.
