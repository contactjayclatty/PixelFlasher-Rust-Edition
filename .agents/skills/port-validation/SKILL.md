---
name: port-validation
description: Validate PixelFlasher Rust parsers, operation plans, native UI and packaged binaries without modifying a real device; use after port changes or before reporting parity.
---

# PixelFlasher port validation

## Procedure

Read docs/PARITY.md and docs/VALIDATION.md. Compare the changed workflow with
the pinned upstream source and source-inventory.json. Run cargo fmt --check,
cargo test --workspace -j 2, and cargo clippy --workspace -j 2 --all-targets -- -D warnings. Build the
native application and capture the affected page using EFRAME_SCREENSHOT_TO and
an isolated PIXELFLASHER_DATA_DIR. Inspect the actual PNG for labels, controls,
contrast and overflow. Verify the CLI on synthetic image/archive inputs.

## Device boundary

Use fake Transport tests for mutations. Real flashing, rooting, wiping or
relocking requires explicit authorization for the selected device and input.
The development task itself authorizes implementing those operations, not
performing them on a connected phone.

## Evidence and failure modes

Keep command exit codes distinct from final phone state. A build is not parity;
a sideload transfer is not a successful recovery update; a root daemon path is
not proof of its fork; a cryptographic signature is not proof of a trusted key.
Unknown prerequisites and pending workflows stay marked as unknown/pending.
Update the matrix and evidence alongside code. Do not weaken failed checks.


For Studio changes, validate both themes and minimum size, reviewed-plan digests,
critical stop checkpoints, schema backups, interrupted journals, provider/method
restrictions, backup device/build/slot/checksums and MCP forbidden tool/field tests.
Package and smoke the stdio binary as well as the preserved-name desktop binary.
Do not mark the approved 1.0 plan complete while IMPLEMENTATION/PARITY has gaps.

For Google catalog changes, test both factory and OTA against the live Rust loader
with explicit terms acknowledgement; do not equate a synthetic URL regex test with
successful table retrieval. Record counts/checksum association, never download
firmware merely to test catalog retrieval. Preserve the default-false consent.
