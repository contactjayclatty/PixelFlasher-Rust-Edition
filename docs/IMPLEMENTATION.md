# Approved 1.0 plan — implementation status

Accepted 10 October 2026. Product: Pixel-first, Windows-first native Rust/egui,
guided automation, expert workspaces, stable defaults, optional local MCP disabled
by default, no built-in/cloud AI. Preserve executable/CLI/library/user data.
No real device mutation is authorized by development work.

| Milestone | Current status | Remaining gates |
|---|---|---|
| 0.3 UI/core | Foundation implemented; shared core, frozen plans, durable journal/migrations, scoped caches, Root/Update tabs, light/dark/density | Complete screen composition, comprehensive keyboard/contrast review, post-stage verified result handling |
| 0.4 Root Studio | Six adapters/pinned pairs, APK identity/version, methods, module controls and manager handoff present | Complete per-version golden APK/daemon/kernel fixtures, compatible GKI assets, reliable native profiles, upgrades/unroot/root-preservation and hardware matrix |
| 0.5 Update/recovery | Existing factory/OTA planners plus new backup sets, checksum manifests and matching restore planning | Verified update/root preservation, rescue state machine, root-data manifests, post-restore verification, disconnect/recovery hardware testing |
| 0.6 Expert/integration | Optional restricted stdio MCP wired to desktop review queue; existing inspection/config/cert tools | Fresh MCP device snapshots, live logcat/update engine, AVB signing/downgrade/alternate APatch, structured editors/full certificate-chain grouping, locales/fonts/RTL |
| 1.0 Full parity | Not complete | Every PARITY row accounted for, supported hardware combinations verified, complete packaging/distribution evidence |

## Retained feature scope

Overview, Update Studio, Root Studio, Firmware & Images, Applications, Backups &
Recovery, Diagnostics and Settings need empty/populated/busy/failed/expert review,
keyboard focus/tooltips and 1024x720 usability. Wireless currently remains a ninth
workspace. Activity must distinguish command completion from verified outcomes.
Device/source-derived drafts must not cross target changes. No hidden effective
wipe/slot/AVB options may be applied: frozen command review remains authoritative.

Firmware scope includes stable/full OTA, beta/canary/watch channels, resumable
checksummed downloads, archive previews and unsupported-format explanations.
Application names/icons, batches, exports and provider-aware privileges remain
in scope. Recovery includes root-data manifests, current-stock matching, slot/
bootloader inspection and bootloop rescue. Advanced scope includes AVB signing/
downgrade, alternate APatch, structured configuration, richer integrity and
certificate-chain analysis. Integrity tools use user-provided configuration and
credentials only; no private attestation-key acquisition or promised verdict.

Root version pairs: Magisk 30.7/30.6; KernelSU 3.3.0/3.2.5; Next 3.4.1/3.4.0;
SukiSU 4.2.0/4.1.3; Wild_KSU 3.1.2/3.1.1; APatch 11224/11219. Matrix fixtures
are planner evidence; hardware combinations remain experimental. New/canary
versions remain unverified. Conflicting manager evidence requires user selection.
Native permission/profile APIs are used only when reliable; otherwise manager
handoff. KernelSU classic vs V2 partition selection must follow official source.

## Completion evidence required

Retain original 45 tests; expand provider/malformed-input/migration coverage;
wrong target/slot/UID/input/prerequisite cases; corrupt downloads/disconnects/
interruptions; MCP bypass/target/secret/shell attempts; real hardware separately
per provider/version/image generation. Formatting, workspace tests, strict Clippy,
release builds, native UI review and packaged smoke checks accompany releases.
Each distribution includes source, hashes, parity and validation documentation.
Existing 0.1/0.2 packages are preserved. macOS/Linux packaging and RTL claims wait
for their own validation. No completed 1.0 release is claimed by this document.
