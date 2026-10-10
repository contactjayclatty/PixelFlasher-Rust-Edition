# Validation — 2026-10-10

Observed development checks:

- `cargo build -j 2`: passed on x86_64-pc-windows-msvc.
- `cargo test -j 2`: 45 tests passed (28 core integration, 11 advanced integration, 6 native workflow tests), plus
  library/binary/doc test targets. No tests were ignored.
- `cargo fmt --check`: passed.
- `cargo clippy -j 2 --all-targets -- -D warnings`: passed.
- Native executable launched and rendered real overview, firmware, flashing,
  root, utilities and settings pages to PNGs in ../artifacts/. Screenshots were
  inspected; the initial theme contrast defect was corrected.
- `cargo build --release -j 2`: passed after final safeguard repairs.
- Packaged `dist/PixelFlasher.exe` passed real executable inspection, nested ZIP
  extraction, persistent-library and read-only legacy-import smoke checks.
  Results and binary SHA-256: ../artifacts/executable-smoke.json.
- Strict Akinator knowledge coverage: exit 0, no findings; report in
  ../artifacts/knowledge-coverage.json. This checks documentation structure, not parity.
- The final packaged release also launched and rendered its real overview page.
- Automatic approval review blocked cleanup of one 8,192-byte development fixture:
  `%TEMP%/pixelflasher-smoke-x_grqdcy/PixelFlasher4.db`. It contains synthetic
  records only and was left in place. The smoke script now explicitly closes
  its Python fixture connection, and subsequent smoke runs cleaned up normally.

Core tests exercise multi-device/mode/property parsing, serial-bound plans,
locked-target rejection, input changes after review, stopped sequences on failure,
slot selection, known-bad SDK releases, boot headers, archive traversal, nested
firmware extraction, OTA hashes and delta rejection, persistent image provenance,
read-only legacy imports, shell identifiers and redaction.
The executor also rejects mismatched/missing/overriding command target selectors
before any transport call. Personal tool command arguments and known echoed
argument values are omitted/redacted at log creation.

Advanced tests cover certificate metadata/issuer verification/revocation lookup,
absence of private-key material in reports, root archive traversal boundaries,
SU policy identifiers, root configuration paths, support ZIP redaction and
native root-patch plans that do not flash.

Not validated: physical device detection, flashing, root patch execution,
bootloader locking/unlocking, OTA recovery completion, module changes, wireless
pairing, scrcpy, downloaded firmware catalogs, online certificate status access,
or non-Windows packaging. No attached phone was flashed or otherwise changed.
The native screenshots use an isolated data directory and contain no fake device.

Fixture tests are evidence about code paths, not an assertion of full feature
parity or hardware safety. See PARITY.md for remaining conversion work.

Independent read-only librarian review initially blocked target-selector
enforcement and personal-tool argv redaction. Both were repaired with regression
tests. The final reinspection returned CLEAR for those repairs and documentation
truthfulness; it did not run hardware operations or certify full application parity.

## Version 0.2 improvements

Regression coverage verifies diagnostic output isolation from editable root
configuration, persistent root preference, selected-serial retention after scans,
disabled-device filtering and stale package-selection clearing. A worker test
proves plan preparation opens review without executing its device command.
JSON5 tests preserve nested/hex values and reject invalid, duplicate or non-finite
values. AVB tests extract a property descriptor and reject overflowing descriptor
lengths. Platform readiness requires both executables, not merely a directory.

UI captures for this batch are in ../artifacts/v02-overview.png and the other
v02 page captures. Final release smoke results remain in executable-smoke.json.
No device commands were executed by the development tests or capture workflow.

Live debug captures exposed duplicate scroll-area IDs in the firmware view.
All scroll areas now have distinct stable IDs; a native rendering regression
checks all nine pages for painted collision warnings. The initial screenshot
showing the defect was replaced after repairing and re-rendering that page.
Version text in support ZIPs and network user agents uses Cargo package metadata.

## Version 0.3 Studio foundation

Workspace suite: 72 tests (9 native UI, 11 advanced, 28 original core, 24 Studio).
Original 45 tests are retained. Added evidence covers immutable digest approval,
secret serialization/redaction, six pinned version pairs, classic/V2 restrictions,
UID/slot mismatches, critical checkpoint stop behavior and actual module-install
plans for all providers. SQLite migration fixtures preserve legacy rows and make
backups; interrupted writes require review, never replay. Backup fixtures reject
changed content/device/build/slot and checksum mismatches. Resume range and catalog
row association parsers reject malformed/mismatched inputs. This is not a real
network download/resume test or hardware boot-image compatibility validation.

The renderer exercises all nine workspaces in empty/populated/busy/failed/expert
states, both themes, at 1024x720 (90 renders). Existing all-page widget-ID warning
regression remains. Render tests are not complete keyboard/accessibility or visual
approval for every populated state. Native empty-page release captures, packaged
CLI and stdio smoke evidence are recorded separately in studio-validation.json.

Independent librarian review found two defects before release: free-form MCP
metadata leaked sensitive labels/descriptions, and Magisk --install-module was
missed by a substring classifier. Metadata exports now use generic/allowlisted
fields; Plan::add explicitly marks destructive privileged stages critical. Both
have regression coverage; final read-only reinspection returned CLEAR. Matching
restores freshly revalidate Android build before entering fastboot; bootloop-only
restoration is still unsupported.

No real phone scan, flashing, rooting, module change, backup/restore, relock or
wireless pairing was performed. No hardware-verified combination is claimed.
All remaining 1.0 gates remain in IMPLEMENTATION.md and PARITY.md.

Final UI coverage uses the production draw method, including activity/error UI.
Provider manager-install fixtures cover synthetic manifest metadata for all twelve
pinned pairs and reject a mismatched selected provider. They do not validate APK
signatures, official binary assets, kernel interfaces or installation on a phone.
Configuration read results reject changed serial/source paths, and target changes
discard pending reviews and sensitive drafts. Frozen review summaries include
lock/unlock wipe semantics and explicit set_active slot transitions.

## Version 0.3.1 catalog regression

Adds three regression tests: default-false/legacy-settings consent and correct
factory/OTA cookie selection; missing acknowledgement rejected before network;
current Google factory/OTA row fixtures retain their checksums. Fixture metadata
comes from https://developers.google.com/android/images and /android/ota, fetched
with their respective acknowledgement cookies. Full pages are development artifacts
only; the source package includes two small table rows, no firmware payloads.
Live Rust executable factory/OTA retrieval, release hashes and native firmware-page
captures are recorded in catalog-validation.json. No phone operation occurs.

Final packaged v0.3.1 verification passed: 75 workspace tests, formatting, strict
Clippy, release build, original CLI smoke and MCP smoke. The Rust executable
retrieved 2,359 factory and 2,121 OTA records, all with SHA-256 values. Native
firmware-page captures in both themes show the acknowledgement control and links.
Independent source/document review returned CLEAR. Counts are a live snapshot,
not a permanent catalog size guarantee; no firmware was downloaded or flashed.

## Shell and catalog table

The device bar is drawn once above the page, with Scan and Inspect. Overview no longer paints the marketing panel or the four-step essay; its three status rows open Update Studio, Firmware & Images, and Settings. Google catalog rows are grouped by the filename model and listed only for the chosen model. Boot images, applications, modules, and partitions use the same table layout. Applications and verified backup sets stay multi-select because those actions apply to a set. Reboot, flash, lock, wipe, and patch stay on their pages and still open a reviewed plan.

`cargo fmt --all` was applied. `cargo test --workspace -j 2` passed (77 tests), including `catalog_file_names_split_into_model_and_build`, `catalog_table_lists_only_the_chosen_model`, the nine-page widget-id check, and both themes at 1024x720. `cargo clippy --workspace -j 2 --all-targets -- -D warnings` passed. Native captures of the empty overview, firmware, applications, root, and backups pages were taken from the debug binary with an isolated `PIXELFLASHER_DATA_DIR` and inspected. A nested grid inside a scroll area painted a black frame; those lists use fixed columns instead. No phone operation was performed.

## Version 0.3.2 — Rust Edition release preparation

The shared core relocation and branding changes pass all 75 workspace tests,
formatting, strict Clippy and Windows release builds. Both packaged binaries pass
the existing CLI/MCP smoke checks. Live factory/OTA metadata retrieval still works;
counts and executable hashes are in release-validation.json. The native Settings
page was captured from the branded release binary and inspected. No phone
operation was performed. Public-file scanning found no private PEM keys or
GitHub/AWS token patterns; public certificate fixtures remain included.

Historical binaries/source packages are preserved locally under dist/archive.
Windows CI is configured separately and does not certify hardware behavior.
Versioned portable/source archives exclude data, cached pages, build directories
and historical binaries. Release publication/remote CI evidence is separate from
these local checks.
