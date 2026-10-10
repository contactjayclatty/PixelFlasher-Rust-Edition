# Optional local MCP

Build: `cargo build --workspace --release -j 2`. The Windows distribution includes
`pixelflasher-mcp.exe`. Enable Local agent integration in desktop Settings first;
it is disabled by default. Configure the client command with the executable's
absolute path. Set PIXELFLASHER_DATA_DIR in both processes for a custom data dir.
There are no cloud-model credentials or network model calls.

Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout; initialize, ping,
tools/list and tools/call. Maximum input line 1 MiB. Supported tools:

- inspect: saved enabled device identifiers/generic labels and library image metadata;
  explicitly stale until locally inspected. No serial/path/secret output.
- capabilities: provider enum (Magisk, KernelSu, KernelSuNext, SukiSu, WildKsu,
  APatch) and version; planner fixtures and hardware-unverified reasons.
- prepare: typed OpenManager or FlashImage intent with known opaque resource IDs.
  Flash targets restricted to boot/init_boot/vendor_boot and matching image kind.
  Root patch intents require local input selection and are currently rejected.
- status: opaque journal IDs and allowlisted status values; descriptive text is
  omitted so credentials in user-authored labels/journals cannot leak. CommandsCompleted does not mean verified root.

Example arguments for prepare:

```json
{"intent":{"kind":"OpenManager","device_id":"<id returned by inspect>","provider":"KernelSuNext"}}
```

Unknown fields, device IDs, artifacts, methods and target overrides are rejected.
There is no execute, approve, arbitrary command or secret endpoint. Queued requests
appear on Overview. Select and inspect the corresponding device, then Prepare
local review; the frozen plan still requires acknowledgement and the typed device
phrase. Starting the server never executes queued work or resumes interrupted jobs.
MCP is not a substitute for hardware validation or a general filesystem server.
