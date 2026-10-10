# Shared core ownership

Read ../../AGENTS.md and ../../docs/ARCHITECTURE.md. This crate has no GUI dependency.
UI, CLI and MCP use these parsers and planners. Preserve reviewed target/hash,
critical-checkpoint and secret boundaries; MCP never gains mutation authority.
Use fake transports and existing integration fixtures; no real device mutation
without explicit device/input authorization. Update parity and validation evidence.
