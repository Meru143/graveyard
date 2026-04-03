lint:
	cargo clippy -- -D warnings

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
