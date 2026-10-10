# PixelFlasher Rust Edition

A native Rust desktop port of [badabing2005/PixelFlasher](https://github.com/badabing2005/PixelFlasher),
with a redesigned device dashboard, firmware/image library, reviewed operation
plans and background device commands. Source revision:
`081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54` (upstream version 10.1.1.1).

**Development port: full upstream feature parity is not complete.** The implemented
workflows have parser/planner tests and native UI captures. Real phone flashing,
root patch execution and wireless/device workflows have not been validated on
hardware. Read [the parity matrix](docs/PARITY.md) before using this build.

## Download

Get the Windows portable ZIP from [GitHub Releases](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases).
Extract it and run `PixelFlasher.exe`. Windows x64 is the validated build target;
Android platform-tools and scrcpy are not bundled. Verify assets with the release's
`checksums.sha256`. Current releases are development prereleases.

## Version 0.3.2 — Rust Edition release

Uses the PixelFlasher Rust Edition name, a conventional shared-core workspace,
Windows CI and versioned portable/source packages. Executable name, CLI commands
and the existing user-data directory are unchanged. See [CHANGELOG.md](CHANGELOG.md).

## Version 0.3.1 — Google catalog repair

Google's download tables require a Devsite acknowledgement cookie. In Firmware &
Images, read the linked factory/OTA terms, tick the acknowledgement checkbox, then
Load Google catalog. The preference starts unchecked and is saved locally.
The application requests the corresponding table and keeps row-associated hashes.
For read-only CLI inspection: `--catalog factory|ota --acknowledge-google-terms`.
This fetches catalog metadata only; it does not download or flash firmware.

## Version 0.3 — Guided Studio foundation

Adds a GUI-independent Rust core, immutable reviewed plans, durable operation
journals and backed-up SQLite migrations. Root Studio has six pinned provider
adapters, APK package/version validation, explicit classic/V2 method restrictions,
manager handoff and separate patch/module/permission/recovery tabs. Update Studio
separates firmware updates from image operations. Dark and light themes have
readable mint accents and compact density.

Multi-partition backup sets compare retrieved images against device-side hashes;
restore plans require matching device, build, slot and unchanged contents.
Patched image lineage records provider version, method and stock SHA-256.
Downloads resume only when the server supplies a matching strong ETag and range.
Optional MCP queues intents for desktop review and cannot execute device changes.
This is a staged implementation, **not the completed 1.0 plan**. Remaining work
and release gates are tracked in [IMPLEMENTATION.md](docs/IMPLEMENTATION.md).

## Run

Open `dist/PixelFlasher.exe`, or build from source:

```powershell
git clone https://github.com/contactjayclatty/PixelFlasher-Rust-Edition.git
Set-Location PixelFlasher-Rust-Edition
cargo run -j 2
```

Select a folder containing both `adb` and `fastboot` in Settings, check their
versions, connect a device, then Scan and Inspect. The app does not contact or
change devices at startup. Every reviewed device operation binds to a selected
serial; flashing additionally checks product and bootloader unlock state.

Firmware processing and image inspection work without a phone. Patch operations
use assets from the chosen root manager APK and the device's detected ABI. Patching
creates a local image; flashing that image is a separate reviewed operation.
KernelSU-Next is the initial root-manager selection; other supported families
are selectable. A root runtime detected at `/data/adb/ksud` is labeled
KernelSU-family because that path alone does not identify the exact fork.

No Python runtime, wxPython, webview or original PixelFlasher process is invoked.
ADB/fastboot, scrcpy and root-manager binaries are external tools where applicable.
No drivers or Android platform tools are bundled in the Windows package.

## Local CLI tools

```powershell
.\target\debug\pixelflasher-rust.exe --inspect C:\path\init_boot.img
.\target\debug\pixelflasher-rust.exe --process C:\path\firmware.zip
.\target\debug\pixelflasher-rust.exe --library
.\target\debug\pixelflasher-rust.exe --import-legacy C:\path\PixelFlasher4.db
```

Legacy import opens the original SQLite database read-only and imports only
existing files whose SHA-1 matches its record. It does not import every legacy
preference or preserve all old database relationships. Missing or changed files
are reported and left untouched.

Default app data: `%LOCALAPPDATA%\PixelFlasherRust` on Windows. Override with
`PIXELFLASHER_DATA_DIR` for isolated fixtures and development. Settings, SQLite
image records, extracted firmware and patch-work directories live there. Keep
that directory private. New APatch scripts read the superkey from stdin; saved
plans omit the secret and require fresh human input after deserialization. Older
v0.1/v0.2 patch folders can contain plaintext keys and are preserved, not rewritten.
Patch scripts are excluded from support exports. Inspect retained temporary
patch files before manually cleaning them up.

## Development and evidence

```powershell
cargo fmt --check
cargo test --workspace -j 2
cargo clippy --workspace -j 2 --all-targets -- -D warnings
cargo build --workspace --release -j 2
```

- [Approved 1.0 plan and milestone status](docs/IMPLEMENTATION.md)
- [Local MCP setup](docs/MCP.md)
- [Contributor instructions](CONTRIBUTING.md)
- [Release process](docs/RELEASING.md)
- [Implemented and remaining workflows](docs/PARITY.md)
- [Architecture and safety boundaries](docs/ARCHITECTURE.md)
- [Build, test and native UI evidence](docs/VALIDATION.md)
- [Decisions and pending work](docs/DECISIONS.md)
- [Upstream function inventory](docs/source-inventory.json)
- [Port validation procedure](.agents/skills/port-validation/SKILL.md)
- [Attribution and license provenance](NOTICE.md)

Native UI capture: set `EFRAME_SCREENSHOT_TO` to a PNG path. Optionally set
`PIXELFLASHER_CAPTURE_PAGE` to `firmware`, `flash`, `apps`, `root`, `backups`,
`wireless`, `utilities` or `settings`. Capturing launches the real application,
renders its current state, saves the screenshot and exits. It does not seed
fictional devices or perform a scan.
