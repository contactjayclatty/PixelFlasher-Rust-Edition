# Changelog

## 0.3.3 — Familiar shell and catalog tables

- Keep the nine destinations and put the device, Scan, and Inspect in one top bar.
- Replace the overview marketing panel with status rows for the device, images, and platform tools.
- Choose a Google firmware model, then one build row, then Download. Factory and Full OTA stay separate.
- Show boot images, applications, modules, partitions, and backup sets as column lists.
- Applications and backup sets still allow several selections. Device changes still require a reviewed plan.

## 0.3.2 — PixelFlasher Rust Edition

- Rename the application, repository documentation and release packages.
- Move the shared backend into crates/core/src while retaining desktop/CLI imports.
- Add Windows CI, contributor/release instructions and clean portable packaging.
- Preserve user data, executable name, existing CLI and previous local packages.

## 0.3.1

- Fix Google factory/OTA catalog retrieval with explicit terms acknowledgement.
- Validate both live catalogs and row-associated SHA-256 values.

## 0.3.0

- Introduce Studio navigation, provider fixtures, frozen operations, durable journals,
  SQLite migrations, patch lineage, backup manifests and restricted local MCP.

All releases remain development previews: full parity and hardware testing pending.
