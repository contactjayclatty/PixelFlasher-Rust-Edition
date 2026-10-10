# Source ownership

Read ../AGENTS.md and ../docs/ARCHITECTURE.md. Native UI code lives in ui.rs and
ui/*.rs; device mutations pass through flash::Plan/execute. Do not bypass the
reviewed target and input-hash boundary when adding a workflow. Verify with the
existing fake-transport regression tests in ../tests/core.rs and fixtures in
../tests/advanced.rs. Keep hardware-unverified paths identified in the parity matrix.
