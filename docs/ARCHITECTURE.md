# Architecture

The UI is eframe/egui with native OpenGL rendering and AccessKit support. A single
background job runs at a time, sending logs and typed results through a channel.
Discovery does not happen until requested. Long operations do not block rendering.
The sidebar replaces upstream's dense toolbar and separate wxPython dialogs.

## Ownership

| Area | Rust source |
|---|---|
| Process invocation, streaming output, timeouts and cancellation | ../crates/core/src/command.rs |
| Device discovery, mode/property parsing and platform-tools checks | ../crates/core/src/device.rs |
| Settings and executable discovery | ../crates/core/src/config.rs |
| Firmware archive processing | ../crates/core/src/firmware.rs |
| OTA protobuf and block extraction | ../crates/core/src/payload.rs |
| Image hashes, Android headers and AVB metadata | ../crates/core/src/image.rs |
| Flash planning, reviewed input snapshots and target preflight | ../crates/core/src/flash.rs |
| APK asset extraction, patch scripts and root module commands | ../crates/core/src/root.rs |
| GKI kernel replacement and repacking | ../crates/core/src/root_kernel.rs |
| Package, download, wireless, partition and root-data workflows | ../crates/core/src/services.rs |
| Certificates, SU policies, backups, diagnostics and support exports | ../crates/core/src/advanced.rs |
| Binary Android XML and build-property conversion | ../crates/core/src/xml.rs |
| SQLite image provenance and read-only legacy import | ../crates/core/src/library.rs |
| Native navigation, background job lifecycle and operation review | ../src/ui.rs |

## Execution boundary

Host commands use `std::process::Command` with argument arrays, not a command
shell. The only intentional shell language is Android-side `adb shell` / `su -c`
and generated root patch scripts. User-supplied identifiers and device paths are
validated or shell-quoted. Factory scripts are parsed into a restricted command
sequence; their shell bodies are never executed.

Each Plan includes a frozen device snapshot, ordered commands and input hashes.
The user reviews it and enters a device-specific phrase. Before execution, hashes
are rechecked. Android target product and fastboot product/unlock state are
validated where required. Known upstream problematic platform-tools releases are
rejected. Failed commands stop the sequence. Cancellation cannot undo prior writes.

The software's checks do not prove firmware trust, kernel compatibility, bootloader
anti-rollback compatibility, or the safety of a modified phone's relock operation.
Those concerns still need the corresponding source workflows and hardware tests.
No destructive operation was performed while developing this port.

## Data and recovery

The Rust library uses a separate SQLite schema and data directory. It never opens
the upstream database for writing. Archive extraction stages are unique, and
corrupt/delta OTA operations fail instead of producing a claimed complete image.
Downloaded content uses HTTPS, partial files and checksums when a checksum is
provided. Partial downloads and failed extraction stages remain inspectable.

Support ZIPs include only redacted settings, session logs and version metadata;
personal-tool arguments and patch scripts are excluded. Root configuration text
is not copied into the activity log. This does not constitute a comprehensive
secret-detection engine for arbitrary personal commands or their output.

Review triggers: changes to upstream command behavior, root manager CLI flags,
OTA operation types, device platform-tools, parser formats or worker lifecycle.

## Guided Studio core (0.3)

The Cargo workspace contains `pixelflasher-core`, the existing desktop/CLI package
and `pixelflasher-mcp`. The root library is a compatibility facade; the core has
no eframe/rfd dependency. UI planners freeze legacy Plan values into immutable
OperationPlan records. Approval binds the frozen digest and device phrase.
Execution uses an exclusive operation lock, persists journal status and rechecks
input hashes and known slots at critical checkpoints. Explicit slot transitions
update the expected slot. Destructive privileged stages are explicitly marked critical by Plan::add,
including Magisk module installation. Critical cancellation waits until the command completes;
timeouts report an exceeded estimate without killing a write. Noncritical platform-tool processes remain
interruptible; preparation checks cancellation between stages. GUI close is deferred while a worker is busy.

`CommandsCompleted` is intentionally distinct from `Verified`. Current device
plans never claim Verified firmware/root success. Interrupted Running journal rows
become Interrupted on restart; no plan is reconstructed or replayed. SQLite v0/v1
migrations make VACUUM INTO backups before modifying existing schemas, then commit
DDL/schema versions transactionally. Version 2 adds typed patch lineage.

APatch secret transport uses memory-held Zeroizing strings and stdin; invocation
serialization skips secrets. Deserialized secret-bearing commands cannot run.
Existing patch scripts from older releases are preserved and may contain keys.

Root adapters own release fixtures, method restrictions and patch-script selection.
Pinned version coverage tests only planners. An installed manager is never proof
of active kernel identity; manager, daemon and kernel evidence remain separate.
Classic workflows cover both boot/init_boot where applicable; V2 is pinned only
for KernelSU 3.3.0 and Next 3.4.1 and rejects init_boot/vendor_boot. The KMI override
is unavailable for V2. Native permission edits remain Magisk-only; other providers
hand off to their managers. Magisk UI policy plans revalidate package UIDs. Root mutation checkpoints verify
current UID 0; read-only root commands remain interruptible.

`recovery.rs` builds reviewed multi-partition backup plans and verifies manifests
before restricted restore planning. Restore execution rechecks the Android build
before entering fastboot; bootloop/fastboot-only restore is currently unavailable. SHA-256 proves transfer/content agreement,
not bootability or trusted firmware. Restores currently require exact build/slot
matching and omit automatic reboot. Root-data archives remain a separate flow.

The MCP service has no Transport or execution access. It reads saved metadata,
prepares typed intents and journals, and uses opaque resource identifiers. The
stdio binary bounds message size and stdout contains JSON-RPC only. Settings gate
all tools on each call. Desktop review re-resolves the current target and artifacts;
there is no approval/execute/shell/secret tool. Current inspection is saved metadata,
not a fresh connected-device snapshot. See MCP.md for the supported contract.

Frozen review summaries show wipe (including bootloader lock/unlock), explicit slot
activation, verity/AVB/force and final reboot choices derived from commands. Target
changes discard reviews and sensitive drafts; configuration read results bind to
serial and source path. The shared draw method is used by both eframe and the
full-frame state renderer, including activity/error UI. Root manager installation
uses the provider adapter's package/version check before preparing adb install.

## Google firmware catalog acknowledgement

The metadata loader obtains explicit consent from its caller and sends the
corresponding `devsite_wall_acks=nexus-image-tos` or `nexus-ota-tos` cookie on the
Google catalog request only. The UI persists a default-false acknowledgement
setting and links both official terms pages; the CLI requires an explicit flag.
Requests have a 45-second timeout, 8 MiB response cap and final-host check. Existing
Google download-host allowlisting and row checksum boundaries remain in place.

## Rust Edition repository layout

Core implementation now lives in crates/core/src with its own instruction router.
The root package keeps the desktop/CLI entry points, ui modules and src/facade.rs
compatibility exports. The optional MCP executable remains crates/mcp. Packaging
uses explicit source/portable allowlists; developer artifacts and rollback builds
are local-only. Windows CI checks formatting, fixtures, Clippy, release builds and
packaged-style CLI/MCP smoke tests without contacting any connected phone.
