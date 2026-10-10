# Contributing

Read AGENTS.md, docs/ARCHITECTURE.md and docs/PARITY.md. Keep hardware-unverified
workflows clearly labeled. Use fixtures and fake transports for device mutations.
Never commit user data, credentials, private keys, platform tools or generated builds.

```powershell
cargo fmt --all --check
cargo test --locked --workspace -j 2
cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings
cargo build --locked --workspace --release -j 2
```

The core lives in crates/core/src, native presentation in src/ui and src/ui.rs,
and optional stdio service in crates/mcp. Existing CLI imports use src/facade.rs.
Release packaging and the publication checklist are in docs/RELEASING.md.
