# Releases

Project: PixelFlasher Rust Edition. Repository: contactjayclatty/PixelFlasher-Rust-Edition.
Development releases are GitHub prereleases while full parity and hardware validation
remain incomplete. The Windows portable executable stays PixelFlasher.exe; the data
directory stays PixelFlasherRust and all existing CLI commands are preserved.

Run the checks in CONTRIBUTING.md, then scripts/package.ps1 and
scripts/package_source.py. scripts/package_release.py creates versioned source and
Windows portable ZIPs, dependency notices and SHA-256 checksums from explicit
allowlists; it does not include user data, cached pages or historical binaries.
Smoke the packaged executables with scripts/smoke.py and scripts/mcp_smoke.py;
validate extraction and record hashes in docs/release-validation.json.

Preserve previous local packages under dist/archive. Publish only the current
versioned release assets. Use a tag matching Cargo version and attach the portable
ZIP, complete source ZIP and checksums.sha256. Keep the source archive and commit
aligned; changing sources after packaging requires repackaging.

Release notes must state full-parity/hardware limits, Windows requirements,
platform-tools availability and default-disabled MCP. No release task authorizes
flashing, root changes, wiping or locking a connected phone.
