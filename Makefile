.PHONY: lint format format-check test test-integration coverage build audit clean install snapshots bench

lint:
	cargo clippy --all-targets -- -D warnings

format:
	cargo fmt

format-check:
	cargo fmt --check

test:
	cargo test --all

test-integration:
	cargo test --test '*'

coverage:
	cargo llvm-cov --html

build:
	cargo build --release

audit:
	cargo audit

clean:
	cargo clean

install:
	cargo install --path .

snapshots:
	cargo insta review

bench:
	cargo bench
