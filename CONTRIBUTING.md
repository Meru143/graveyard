# Contributing

`graveyard` is a Rust CLI with a strict quality gate. Changes should land with tests, pass the documented checks locally, and keep the command-line interface aligned with the PRD and TODO documents in the repository root.

## Development Setup

Install Rust 1.75 or newer, Git, and the platform toolchain required to build `git2` with vendored `libgit2`. Python and Node are only needed when working on the release packaging paths.

```bash
git clone https://github.com/you/graveyard.git
cd graveyard
cargo build
```

The main day-to-day targets are exposed through the `Makefile`:

```bash
make format
make format-check
make test
make build
make audit
```

## Local Verification

Run the full gate before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
cargo audit
cargo deny check
```

If you touch output renderers or snapshots, review and update them intentionally:

```bash
make snapshots
```

If you change coverage-sensitive logic or CI configuration, verify the coverage pipeline locally:

```bash
cargo llvm-cov --lcov --output-path lcov.info
```

## Pull Request Process

Keep each pull request scoped to one logical change. Update tests and documentation in the same branch when behavior changes. Conventional commits are used throughout the repository, so prefer messages such as `feat: add rust symbol extractor`, `fix: handle shallow clone scoring`, or `docs: expand ci integration guide`.

Pull requests should describe the user-visible behavior change, the verification commands you ran, and any risks or follow-up work. If a change affects parsing, scoring, CI, or packaging, include a note about the fixture or workflow coverage that protects it.

## Project Expectations

`graveyard` is read-only by design. Do not add auto-delete behavior, editor integrations, network services, or language targets that are explicitly listed as future scope in the PRD. Keep the implementation aligned with the current command surface and distribution model unless the TODO or PRD changes first.
