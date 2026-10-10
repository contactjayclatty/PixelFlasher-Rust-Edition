# Feature parity

Reference: upstream 081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54, version 10.1.1.1.
The [function inventory](source-inventory.json) records 61,527 Python lines and
1,335 top-level/class functions across root Python modules, including generated
image/protobuf code and standalone AVB tooling. Function counts are not a parity
percentage. Full source-to-source behavioral parity is not yet achieved.
Regenerate the inventory with `python scripts/source_inventory.py` using the
pinned reference checkout; that script is development tooling, not app runtime.

`Implemented` means the Rust path exists and is wired to the UI or documented CLI.
It does not mean device execution has been verified. All device mutation paths
below remain hardware-unverified. Pure functionality and selected plans have
automated fixture coverage; see [VALIDATION.md](VALIDATION.md).

| Upstream area | Rust path | Status and limits |
|---|---|---|
| Native app, normal/expert controls, activity console | ../src/ui.rs | Implemented: readiness checklist, drag/drop firmware, searchable image library, collapsed activity panel and background plan preparation; live-render captures inspected |
| ADB/fastboot discovery, multiple devices, connection modes | ../crates/core/src/device.rs | Implemented parsers and transport; no live-device scan in development |
| Properties, firmware, slots, root runtime, battery diagnostics | ../crates/core/src/device.rs, ../src/ui/pages.rs | Implemented; exact KernelSU fork cannot be inferred from daemon path alone |
| Platform-tools selection/version warnings | ../crates/core/src/config.rs, ../crates/core/src/device.rs | Implemented; known upstream 34.0.0–34.0.4 guard preserved |
| Persistent labels/enabled device registry | ../crates/core/src/config.rs, ../src/ui/advanced_pages.rs | Implemented: saved labels and enabled filtering, optional scan of disabled devices, serial-based selection retention |
| Boot/init_boot/vendor_boot library, checksums, provenance | ../crates/core/src/library.rs, ../crates/core/src/image.rs | Implemented; imported images need explicit product association outside legacy import |
| Read-only legacy BOOT database import | ../crates/core/src/library.rs | Implemented; original dates, package relationships and all preferences not migrated |
| Factory ZIP and nested image ZIP extraction | ../crates/core/src/firmware.rs | Implemented, fixture-tested; Samsung Odin/AP tar handling not equivalent to upstream |
| Full OTA payload extraction | ../crates/core/src/payload.rs | Implemented REPLACE/BZ/XZ/ZERO/DISCARD and hashes; incremental OTA explicitly rejected |
| Boot headers and AVB footer/header inspection | ../crates/core/src/image.rs | Implemented metadata subset; AVB property descriptors parsed with bounded lengths; signature verification, other descriptors and kernel strings remain incomplete |
| Factory flash, keep/wipe data, slots, root-image override | ../crates/core/src/flash.rs | Implemented plans and target checks; root preservation and actual flashing unverified |
| Standalone image flash and temporary live boot | ../crates/core/src/flash.rs | Implemented; live boot restricted to boot images |
| Full OTA sideload | ../crates/core/src/flash.rs | Implemented command plan; recovery result and adb nonzero-success edge cases need hardware validation |
| Reboot system/bootloader/recovery/fastbootd/sideload | ../src/ui/pages.rs | Implemented reviewed commands; interactive recovery, safe/download mode variants incomplete |
| Bootloader lock/unlock and slot changes | ../src/ui/pages.rs | Implemented reviewed commands; requires device confirmation; ARB-aware slot inspection still pending |
| Root APK release discovery/download/install | ../crates/core/src/providers.rs, ../crates/core/src/services.rs | Implemented GitHub assets and provider-validated stable manager installer plans; prerelease/channel handling differs from upstream |
| Magisk, KernelSU/Next, SukiSU, Wild_KSU, APatch patching | ../crates/core/src/root.rs | Partial: six pinned adapters validate APK identity/version and classic/V2 targets; scripts remain hardware-unverified, complete native profile/provider upgrade support pending |
| GKI kernel replacement | ../crates/core/src/root_kernel.rs | Implemented manual kernel selection/repack plan; automatic KMI-compatible release asset selection pending |
| APatch manual alternate patching and legacy special Magisk recovery builds | — | Pending |
| Module list, enable/disable/remove/restore/action/install/SOS disable | ../crates/core/src/root.rs | Implemented Android-mode plans; bootloop-special recovery automation pending |
| Magisk Zygisk/denylist and per-app SU policy | ../crates/core/src/advanced.rs, ../src/ui/advanced_pages.rs | Implemented subset; exact options and permissions for other managers incomplete |
| Package list/filter/multiselect/export/enable/disable/uninstall/clear/kill/launch/details | ../crates/core/src/services.rs, ../src/ui/pages.rs | Implemented; app label/icon extraction and richer permission inspection pending |
| APK install and split/base APK retrieval | ../crates/core/src/services.rs | Implemented; alternate installer flags exposed in Expert controls; device execution unverified |
| Wireless pair/connect/disconnect | ../crates/core/src/services.rs | Implemented; paired-service discovery/history UI pending |
| Logcat snapshots and clear | ../src/ui/pages.rs | Implemented; continuous filtered logcat/update-engine windows pending |
| scrcpy and Shizuku launch | ../src/ui/pages.rs | Implemented command integration; actual execution unverified |
| Partition listing/dump/erase and /data/adb backup/restore | ../crates/core/src/services.rs | Implemented with archive traversal checks; multi-partition sets and checksum-bound manifests implemented; matching restore planning implemented; verify-after-restore and bootloop automation pending |
| Magisk stock backup list/add/delete | ../crates/core/src/advanced.rs | Implemented; auto-match/current-backup highlighting pending |
| Root configuration editor and common PIF/TrickyStore paths | ../crates/core/src/services.rs, ../src/ui/pages.rs | Implemented JSON/JSON5/text editing; duplicate/non-finite values rejected, .json writes normalized; structured TargetedFix operations, push keybox and module auto-detection pending |
| Device integrity diagnostics report | ../crates/core/src/advanced.rs | Implemented subset; upstream's full diagnostic coverage not matched |
| Automated integrity checker UI, beta/canary/random PIF generation | — | Pending |
| Keybox certificate dates/signatures/status lookup | ../crates/core/src/advanced.rs | Implemented; AOSP/issuer heuristics and multiple independent key-chain grouping incomplete |
| Binary XML/APK manifest and build props to JSON | ../crates/core/src/xml.rs, ../crates/core/src/advanced.rs | Implemented parser subset; protobuf ABX/packages.xml formats and typed-dimension rendering pending |
| AVB downgrade preparation, signing and no-wipe downgrade workflow | — | Pending; not represented by a cosmetic enabled button |
| Personal executable tools | ../crates/core/src/config.rs, ../src/ui/pages.rs | Implemented basic recipes; full directory/reordering/editor options pending |
| Support ZIP and log export | ../crates/core/src/advanced.rs | Implemented with selected redaction; no full upstream support archive compatibility |
| Firmware beta/canary/watch catalog refresh and automatic program/module updates | — | Pending beyond current Google stable factory/OTA catalogs and manual GitHub release browsing |
| Configurable toolbar and all advanced preferences | — | Replaced by sidebar and limited settings; no complete preference parity |
| 23 upstream translation catalogs | — | Pending; Rust UI currently English |
| Windows/macOS/Linux distribution | Cargo.toml | Windows build/run checked; other targets not built or packaged |

Hardware validation must use explicit user approval for a specific target/image.
Fixture tests never execute real flashing, rooting, relocking, wiping or network
pairing. Pending rows remain part of the requested full conversion.

## 0.3 plan delta

| Area | State and evidence |
|---|---|
| Shared core, desktop facade and optional stdio MCP | Implemented; Cargo workspace and agent.rs; MCP inspection currently cached metadata |
| Frozen plans and local approval | Implemented; UI preparation/run_pending paths; digest and target tests |
| Journal, interruption recovery, schema backups and lineage | Implemented; library.rs/operation.rs; migration and no-replay fixtures |
| Critical cancellation, input/slot checkpoint checks | Implemented; operation.rs/command.rs; wrong-slot and deferred-stop fixtures; hardware unverified |
| Graphite/mint light/dark, density, workspace tabs | Partial redesign; Root/Update tabs wired; existing expert pages still require further composition work |
| Six provider fixtures and APK package/version validation | Planner matrix implemented; equal hardware validation not performed; new/canary releases rejected as unverified |
| KernelSU V2 target restrictions | Implemented for 3.3.0 / Next 3.4.1 only; source-confirmed, output execution unverified |
| Native permission/profile controls | Magisk UI revalidates UID; other providers use official-manager handoff |
| Multi-partition backup/manifest/restore planning | Implemented and fixture-tested; exact device/build/slot and content checks; no verified real restore |
| Resumable firmware downloads | Implemented strong ETag/Range checks and row-scoped checksum association; real network resume still unverified |
| Root preservation, native provider upgrades, complete AVB/downgrade, live diagnostics, translations | Pending; accepted scope remains in IMPLEMENTATION.md |

This release is not v1.0 and does not satisfy every row of the approved plan.

### 0.3.1 catalog repair

The desktop catalog is a model selector and a three-column table (file, build, SHA-256) with one selected row and one Download action. Root-manager release assets use that same table. Retrieval behavior is unchanged.

Google factory/full-OTA table retrieval now supplies the appropriate Devsite
acknowledgement cookie after explicit caller consent. Factory/OTA row fixtures
retain source SHA-256 values. Live packaged-executable verification is recorded
in catalog-validation.json; metadata retrieval is separate from firmware download
and device validation. Beta/canary/watch coverage remains pending.

### 0.3.2 Rust Edition publication

Application branding and shared-core layout updated; runtime workflows are retained.
Public Windows/source prerelease packaging and CI added. Local checks and release
asset hashes live in release-validation.json. GitHub CI status is separate from
local validation. Full parity, other-platform builds and hardware testing remain
pending; publication is not a completion claim for the 1.0 roadmap.
