<div align="center">

# PixelFlasher Rust Edition

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL_3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![CI](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/actions/workflows/ci.yml/badge.svg)](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/contactjayclatty/PixelFlasher-Rust-Edition?include_prereleases&label=version)](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases)
[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)

**A native Rust desktop companion for Pixel firmware and device workflows.**

Review the operation. Confirm the target. Only then make a change.

[Download the latest release](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases) ·
[Feature parity](docs/PARITY.md) ·
[Changelog](CHANGELOG.md) ·
[Report a problem](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/issues)

<img src="docs/screenshots/overview.png" alt="Overview with device controls, Scan, Inspect, and status rows" width="920">

</div>

> [!WARNING]
> **Development preview — not hardware-validated.** This is an independent Rust port of [PixelFlasher](https://github.com/badabing2005/PixelFlasher), based on upstream revision `081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54` (10.1.1.1). It is not a drop-in replacement and does not yet implement every upstream feature. Automated tests cover parsers, planners, and selected workflows; they do not prove that flashing, rooting, recovery, or other device operations work on real hardware. Read [the feature-parity matrix](docs/PARITY.md) and [validation record](docs/VALIDATION.md) before using a device.

## What is PixelFlasher Rust Edition?

PixelFlasher Rust Edition is a native desktop application for organizing Pixel firmware and images, inspecting device state, and preparing device operations for review. It uses Rust, egui, and Android platform-tools. The Rust app does **not** launch or embed the upstream Python application.

The interface is organized around a persistent device bar and nine workspaces: Overview, Update Studio, Root Studio, Firmware & Images, Applications, Backups & Recovery, Diagnostics, Settings, and Wireless. Background jobs keep the interface responsive; device discovery starts only when you request a scan.

## Highlights

| Area | What you can do |
|---|---|
| **Firmware & images** | Organize boot-related images, inspect image metadata, and work with Factory and Full OTA firmware catalogs. Rows show build information and SHA-256 values where available. |
| **Update and flash planning** | Prepare factory, OTA, standalone-image, and live-boot operations. Review the frozen commands, target, and relevant choices before confirming. Live boot is restricted to boot images. |
| **Root Studio** | Prepare workflows for Magisk, KernelSU, KernelSU-Next, SukiSU, Wild_KSU, and APatch; manage supported module and manager actions. Provider and version limitations apply. |
| **Applications and utilities** | Browse packages, prepare supported package actions, pair wireless debugging, inspect logs, and launch integrations such as scrcpy and Shizuku. These workflows have not been validated on a connected phone. |
| **Backups and recovery** | Create supported partition backups and checksum-bound manifests. Restore planning checks the device, build, slot, and content; matching does not prove that restored data will boot. |
| **Local library** | Keep image files, checksums, and provenance in the Rust Edition library. Legacy BOOT database import is read-only and only imports files whose recorded SHA-1 still matches. |
| **Optional local MCP** | Let a local agent inspect saved metadata and queue a limited request for desktop review. MCP is off by default and cannot execute device operations, run a shell, or access secrets. |

This is a snapshot of implemented paths, **not a promise of complete upstream parity**. Some features are partial, some are pending, and implemented device operations remain unverified on hardware. See the [full parity matrix](docs/PARITY.md) for exact limits.

### A look around

| Firmware & Images | Root Studio |
|---|---|
| <img src="docs/screenshots/firmware.png" alt="Firmware catalog with Factory and Full OTA choices, a model selector, and build rows" width="440"> | <img src="docs/screenshots/root.png" alt="Root Studio showing provider selection and manager assets" width="440"> |
| Choose a catalog, select a model, review a build row, then choose an action. | Select a provider and prepare its supported workflow for review. |

## Download and install

The published portable application currently targets **Windows x64**. macOS and Linux are not built or packaged, and have not been validated.

1. Open [Releases](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases) and download the latest Windows x64 ZIP and `checksums.sha256`.
2. Verify the ZIP's SHA-256 against the matching entry in `checksums.sha256`.
3. Extract the ZIP and launch `PixelFlasher.exe`.
4. Install or locate Android [platform-tools](https://developer.android.com/tools/releases/platform-tools) separately. In **Settings**, select the folder containing both `adb.exe` and `fastboot.exe`.

Platform-tools and scrcpy are **not** included in the application ZIP. The app also does not bundle the original Python application.

By default, settings and the image library are stored in:

```text
%LOCALAPPDATA%\PixelFlasherRust
```

Set `PIXELFLASHER_DATA_DIR` before launching the app to use a separate data directory. Keep backups of data you care about; changing this location selects a different workspace.

## First session

1. Start the application. It does not scan for or communicate with a device automatically.
2. Open **Settings** and select the platform-tools folder. The app checks for both ADB and fastboot.
3. Connect your phone in the state required for the task, then select **Scan**. Select the intended device before inspecting or preparing an operation.
4. In **Firmware & Images**, add a supported firmware or image file, or choose a Google Factory/Full OTA catalog. Catalog access requires explicit acknowledgement of Google's terms.
5. Read each proposed operation carefully. Check the selected device, input files, commands, and choices such as wipe or slot changes. A device-changing action requires a reviewed plan and its device-specific typed confirmation.
6. Do not proceed unless you understand the operation and have a recovery path for your device.

The confirmation flow is a safeguard, not a guarantee of firmware authenticity, bootability, compatibility, anti-rollback safety, or successful recovery. A command that completed is not necessarily a verified result. Cancellation cannot undo a write that has already completed.

## Safety and validation

Device operations can erase data, prevent a device from booting, or require specialized recovery. In particular, flashing, bootloader unlock/lock, root changes, partition operations, and restores can have serious consequences.

The application prepares a frozen operation plan with the selected target and relevant input hashes. Before running, it rechecks important inputs and target conditions; destructive work remains subject to review and confirmation. Interrupted operations are recorded for inspection and are **not** replayed automatically.

These safeguards do not establish that:

- an image is trustworthy, compatible, or safe to flash;
- a device operation will succeed or leave the phone bootable;
- a restored partition or root workflow is correct for your specific device;
- every upstream PixelFlasher feature is present; or
- a tested planner fixture represents real-device testing.

**No real device mutations are authorized or performed as part of development.** Flashing, rooting, unlocking, relocking, wiping, wireless pairing, and restore paths have not been validated on hardware. Review [validation evidence and gaps](docs/VALIDATION.md), [architecture and execution boundaries](docs/ARCHITECTURE.md), and [parity status](docs/PARITY.md).

## Command-line use

The desktop executable also supports a small set of non-interactive commands. Run them from PowerShell with the path to the executable or from a directory containing it:

```powershell
.\PixelFlasher.exe --inspect .\images\init_boot.img
.\PixelFlasher.exe --process .\firmware\factory-image.zip
.\PixelFlasher.exe --library
.\PixelFlasher.exe --import-legacy .\PixelFlasher4.db
.\PixelFlasher.exe --catalog factory --acknowledge-google-terms
.\PixelFlasher.exe --catalog ota --acknowledge-google-terms
```

- `--inspect` reads image metadata; it does not flash the image.
- `--process` processes a supported firmware archive; incremental OTA payloads are rejected.
- `--library` inspects the local Rust Edition library.
- `--import-legacy` performs a read-only legacy import. It does not modify the source database.
- `--catalog` retrieves catalog metadata after the explicit Google-terms acknowledgement. It does not download firmware or communicate with a device.

These commands do not replace the reviewed desktop workflow for device operations. See [MCP documentation](docs/MCP.md) for the separate optional local-agent interface and its restrictions.

## Build from source

Install the Rust toolchain and platform dependencies for eframe, then from the repository root:

```powershell
git clone https://github.com/contactjayclatty/PixelFlasher-Rust-Edition.git
cd PixelFlasher-Rust-Edition
cargo run -j 2
```

Run the repository's workspace checks and release build with:

```powershell
cargo fmt --all --check
cargo test --locked --workspace -j 2
cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings
cargo build --locked --workspace --release -j 2
```

Windows CI runs automated checks; CI status is not evidence of device compatibility. To capture the native interface during development:

```powershell
$env:EFRAME_SCREENSHOT_TO = "screenshot.png"
$env:PIXELFLASHER_CAPTURE_PAGE = "firmware" # for example: overview, root, apps, backups
cargo run --release
```

See [Contributing](CONTRIBUTING.md) for code layout and test expectations, and [Releasing](docs/RELEASING.md) for packaging and publication rules.

## Optional local MCP

The separate `pixelflasher-mcp` executable uses newline-delimited JSON-RPC over standard input/output. Enable local agent integration in desktop **Settings**; it is disabled by default. It can inspect saved, potentially stale metadata and queue a limited typed request for local desktop review. It has no device transport or execution capability, and exposes no approval, arbitrary shell, or secret-reading tool. Starting MCP never executes queued work.

See [MCP protocol, tools, and setup](docs/MCP.md) before enabling it.

## Documentation

| Document | Description |
|---|---|
| [Feature parity](docs/PARITY.md) | Implemented, partial, hardware-unverified, and pending upstream areas |
| [Architecture](docs/ARCHITECTURE.md) | Core/UI boundaries, plans, execution controls, data handling |
| [Implementation plan](docs/IMPLEMENTATION.md) | Approved roadmap and completion gates |
| [Validation](docs/VALIDATION.md) | Automated evidence, UI captures, and unvalidated workflows |
| [MCP](docs/MCP.md) | Optional local MCP setup and supported contract |
| [Contributing](CONTRIBUTING.md) | Repository layout and checks for contributors |
| [Releasing](docs/RELEASING.md) | Release packaging and publication checklist |
| [Changelog](CHANGELOG.md) | Changes by release |
| [Notice](NOTICE.md) | Upstream attribution and license notes |

The shared core is in `crates/core`; the desktop presentation and compatibility facade are in `src`; the optional MCP executable is in `crates/mcp`.

## Project status and upstream relationship

The upstream application is a Python/wxPython project. Rust Edition is an independent port with a native desktop interface and a different execution-safety model. The current release is a development preview, not a complete conversion or an assurance of behavioral equivalence. The [approved implementation plan](docs/IMPLEMENTATION.md) describes the roadmap to 1.0; the [parity matrix](docs/PARITY.md) tracks current feature coverage.

PixelFlasher Rust Edition is licensed under AGPL-3.0-or-later. Upstream attribution and the license discrepancy record are in [NOTICE.md](NOTICE.md). Pixel is a trademark of Google; this project is not affiliated with Google.

## Get help

- Check the [parity matrix](docs/PARITY.md) and [validation record](docs/VALIDATION.md) for known gaps.
- Search [existing issues](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/issues) or [open a new issue](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/issues/new).
- Include the app version, Windows version, relevant workflow, and sanitized logs when reporting a problem. Remove serials, personal paths, tokens, and other sensitive information before sharing logs.

<div align="center">

[Releases](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/releases) ·
[Issues](https://github.com/contactjayclatty/PixelFlasher-Rust-Edition/issues) ·
[Parity](docs/PARITY.md)

</div>
