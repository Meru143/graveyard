# Contributing

`graveyard` is a Rust CLI with a strict quality gate. Changes should land with tests, pass the documented checks locally, and keep the command-line interface aligned with the shipped documentation and workflow contracts in this repository.

## Development Setup

Install Rust 1.75 or newer, Git, and the platform toolchain required to build `git2` with vendored `libgit2`. Python and Node are only needed when working on the release packaging paths.

```bash
git clone https://github.com/Meru143/graveyard.git
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

The repository uses two GitHub Actions workflows. `CI` runs for pull requests targeting `main` and for pushes to `main`, and branch protection only requires the final summary job named `CI`. `Release` runs only for semver tags such as `v0.1.2` or through manual dispatch, so ordinary commits do not publish packages.

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

Avoid pushing directly to `main` unless you are intentionally updating protected branch state as a maintainer. The normal path is a pull request so the full matrix runs once on the branch and once on the merged `main` commit.

## Project Expectations

`graveyard` is read-only by design. Do not add auto-delete behavior, editor integrations, network services, or language targets that are out of scope for the current release. Keep the implementation aligned with the current command surface and distribution model unless the documented project scope changes first.
