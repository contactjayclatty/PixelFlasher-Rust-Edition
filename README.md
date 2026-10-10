# PixelFlasher Rust Edition

<div align="center">

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL_3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![CI](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/actions/workflows/ci.yml/badge.svg)](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/actions/workflows/ci.yml)
[![Version](https://img.shields.io/github/v/release/contactjayclatty/PixelFlasher-Rust-Edition?include_prereleases&label=version)](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases)
[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Stars](https://img.shields.io/github/stars/contactjayclatty/PixelFlasher-Rust-Edition?style=social)](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/stargazers)

**A fast, native Rust desktop application for flashing, rooting, and managing Google Pixel phones.**

Modern egui interface • Reviewed operation plans • Shared Rust core • Windows-first

</div>

---

> **⚠️ Development Preview (v0.3.3)**
>
> This is an active port of [badabing2005/PixelFlasher](https://github.com/badabing2005/PixelFlasher) (reference revision `081286d6…`, upstream v10.1.1.1).  
> Full feature parity is **not** complete. Device flashing, rooting, and wireless workflows have **not** been validated on real hardware.  
> Parser, planner, and UI tests exist; real-device execution does not.  
> **Read [docs/PARITY.md](docs/PARITY.md) before use.** Use at your own risk.

---

## Why Rust Edition?

| | Original PixelFlasher | Rust Edition |
|---|---|---|
| Language | Python + wxPython | Rust + egui / eframe |
| Startup | Python runtime | Native binary |
| UI | Classic desktop | Modern, themed, responsive |
| Safety model | Direct commands | Frozen reviewed plans + journals |
| Core | Monolithic | Shared `pixelflasher-core` + desktop facade |
| MCP | — | Optional local stdio MCP (review-only) |

The goal is a clean, performant, safety-focused native rewrite that preserves the proven workflows while improving the experience.

## Features (current)

- **Device dashboard** with multi-device support, labels, readiness checks, and background scans
- **Firmware & Images** — factory ZIP processing, OTA payload extraction, image library with SHA-256 provenance
- **Guided Studios** — Root Studio (Magisk, KernelSU/Next, SukiSU, Wild_KSU, APatch) and Update Studio
- **Reviewed operation plans** — every mutation is frozen, hashed, and requires explicit confirmation
- **Durable journals** and SQLite migrations with automatic backups
- **Multi-partition backup/restore planning** with content verification
- **Google catalog** (factory + full OTA) with explicit terms acknowledgement, a model selector, and one selected build row
- **Familiar shell** — device, Scan, and Inspect stay above every page; long lists use columns
- **Light / dark themes** with mint accents and compact density
- **CLI tools** for inspect, process, library, and legacy import
- **Optional local MCP** that can only queue intents for desktop review (never executes changes)

See the full status matrix in [docs/PARITY.md](docs/PARITY.md) and the approved 1.0 plan in [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md).

## Download

Windows x64 portable ZIP (development prerelease):

**→ [GitHub Releases](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases)**

1. Download the ZIP and `checksums.sha256`
2. Verify the hash
3. Extract and run `PixelFlasher.exe`

Android platform-tools (`adb` + `fastboot`) and scrcpy are **not** bundled. Point the app at your own copy in Settings.

## Quick Start

```powershell
# From source (Windows)
git clone https://github.com/contactjayclatty/PixelFlasher-Rust-Edition.git
cd PixelFlasher-Rust-Edition
cargo run -j 2
```

1. In **Settings**, select the folder containing both `adb.exe` and `fastboot.exe`.
2. Connect a device (USB or wireless).
3. Use **Scan** to discover devices — nothing is contacted at startup.
4. Explore Firmware & Images, Root Studio, or Applications.
5. Any operation that changes the device produces a **reviewed plan**. Confirm the exact phrase before it runs.

**Data directory:** `%LOCALAPPDATA%\PixelFlasherRust`  
Override with `PIXELFLASHER_DATA_DIR` for testing.

## CLI

```powershell
pixelflasher-rust.exe --inspect path\to\init_boot.img
pixelflasher-rust.exe --process path\to\firmware.zip
pixelflasher-rust.exe --library
pixelflasher-rust.exe --import-legacy path\to\PixelFlasher4.db
pixelflasher-rust.exe --catalog factory --acknowledge-google-terms
```

Legacy import is read-only and only copies files whose SHA-1 still matches.

## Development

```powershell
cargo fmt --check
cargo test --workspace -j 2
cargo clippy --workspace --all-targets -j 2 -- -D warnings
cargo build --workspace --release -j 2
```

Windows CI runs the same checks on every push/PR.

**Useful docs**

- [Architecture & safety boundaries](docs/ARCHITECTURE.md)
- [Feature parity matrix](docs/PARITY.md)
- [1.0 implementation plan](docs/IMPLEMENTATION.md)
- [MCP contract](docs/MCP.md)
- [Validation evidence](docs/VALIDATION.md)
- [Contributing](CONTRIBUTING.md)
- [Releasing](docs/RELEASING.md)
- [Changelog](CHANGELOG.md)
- [Attribution & licenses](NOTICE.md)

### Screenshot capture

```powershell
$env:EFRAME_SCREENSHOT_TO = "screenshot.png"
$env:PIXELFLASHER_CAPTURE_PAGE = "root"   # or firmware, flash, apps, …
cargo run --release
```

## Attribution

This is an independent development port of **PixelFlasher** by [badabing2005](https://github.com/badabing2005/PixelFlasher) and contributors.  
Upstream reference: `081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54` (v10.1.1.1).  
The Rust implementation does not embed or invoke the original Python application.  
See [NOTICE.md](NOTICE.md) for full license provenance.

**Pixel** is a trademark of Google. This project is not affiliated with Google.

## Disclaimer

Flashing, rooting, and bootloader operations can permanently brick or wipe your device.  
This software is provided **as-is** with no warranty. You are solely responsible for any outcome.  
Always have a way to recover before experimenting. Hardware validation of this port is still pending.

---

<div align="center">

Made with Rust • Port of a community classic

[Report an issue](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/issues) • [Releases](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases)

</div>
