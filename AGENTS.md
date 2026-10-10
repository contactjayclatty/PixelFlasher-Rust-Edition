# PixelFlasher Rust Edition

Native Rust source port of upstream revision 081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54.
Read README.md, docs/PARITY.md and docs/ARCHITECTURE.md before changes.
Preserve upstream attribution and the license discrepancy recorded in NOTICE.md.
No Python interpreter or upstream application is invoked by the Rust runtime.
Device transport uses external Android platform-tools, as upstream does.

Keep parsers pure and testable. Device mutations require a reviewed operation
plan, explicit target selection, typed confirmation and preflight validation.
Never execute flashing, unlocking, wiping or root modifications during development
without the user's explicit authorization for that specific connected device.
Use synthetic inputs and fake transport tests for destructive workflows.
Preserve secrets and serials in exported logs through redaction.
Run cargo fmt --check, cargo test -j 2, cargo clippy -j 2 --all-targets -- -D warnings.
Build with cargo build -j 2. Capture the actual native UI to verify layout.
Update feature parity and validation evidence alongside implementation changes.

Read docs/IMPLEMENTATION.md and docs/MCP.md before continuing the approved 1.0 plan.
Use --workspace for test/Clippy/release builds; include crates/ in source packages.
Never add MCP approval/execution/shell/secret tools. Planner fixtures are not
hardware verification. Extend provider-specific methods and preserve old packages.
Always specify UTF-8 when scripts read/write sources or documentation on Windows.

The shared core is owned by crates/core/src; src is desktop presentation plus the
compatibility facade. Follow CONTRIBUTING.md and docs/RELEASING.md for release work.
Public packaging uses allowlists; exclude generated data, secrets and cached pages.
Preserve local rollback packages in dist/archive; preserve the executable/data names.
