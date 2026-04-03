## Summary

Describe the change and the user-visible behavior it introduces or modifies.

## Verification

List the commands you ran and their results.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
```

## Checklist

- [ ] Tests were added or updated where behavior changed.
- [ ] Documentation was updated where behavior changed.
- [ ] CI, packaging, or release workflow changes were validated locally when applicable.
- [ ] The branch uses a conventional commit message.
