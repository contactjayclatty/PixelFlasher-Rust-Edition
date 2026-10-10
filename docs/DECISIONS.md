# Decisions — 2026-10-10

The owner requested a full Rust conversion of PixelFlasher with a better UI.

The full objective remains open; this development build implements a substantial

native slice with a source-referenced parity matrix. No missing workflow is

represented as complete merely because the app builds or a button exists.

Native egui was chosen over an embedded browser to keep the application and

business logic in Rust, avoid a Python bridge and provide a self-contained native

window. The sidebar, image library and operation-review window separate navigation,

input preparation and actual device execution. The original checkout is a pinned

reference under ../references/PixelFlasher from the collection root.

Knowledge delta: README.md, AGENTS.md, NOTICE.md, docs/ARCHITECTURE.md,

docs/PARITY.md, docs/VALIDATION.md, docs/source-inventory.json, this decision file,

and the local port-validation skill. The root collection README links this project.

Structural facts belong in Rust modules and the generated source inventory; the

parity matrix tracks workflow gaps instead of duplicating all function bodies.

KernelSU-Next is the initial UI choice, consistent with the owner's recorded root

preference. This does not assume the current device or root runtime. Actual device

mode, ABI and root state must be inspected. No device serial, IMEI, credential or

recovery code is included in repository documentation or fixtures.

Native process argv avoids host-shell interpretation. Explicit reviews, frozen

device/input plans, typed confirmation and preflight checks were added because an

incorrect flashing target is not a reversible UI mistake. Pure parsers and fake

transport tests validate these checks without modifying a phone.

A first live render exposed system-theme overrides producing dark text on dark

panels; explicit theme preference fixed it. A certificate fixture initially lacked

its issuer, so chain status correctly remained unknown; a separate issuer-chain

fixture now tests mathematical signature verification. Unknown chain state was

kept distinct from a verified signature.

Independent review found that public Plan fields allowed a reviewed target to

differ from a command's serial, and personal-tool arguments could enter support

logs. Executor-level selector validation and early argv/output redaction fixed

both; regression tests reject target overrides before any transport call.

The port uses a separate data schema and read-only legacy imports to preserve the

original app's data. Source ZIP/database provenance cannot prove firmware trust or

an image's compatibility with a selected device. Device workflows therefore need

hardware testing in addition to unit tests.

Review these decisions when completing pending parity rows, changing the native

UI framework, migrating old configuration, upgrading root tools, or validating

additional operating systems. No application was deployed or published.

## Version 0.2 — workflow and UI refinement

User requested further improvement while full conversion remains the overall

objective. Knowledge delta: this note, PARITY.md, VALIDATION.md and README.md;

code changes own the current readiness, parsing and result-routing behavior.

The activity panel starts collapsed to prioritize the actual update workflow;

long hash/patch preparation runs on a worker and returns a review-only payload.

Certificate/diagnostic output uses a distinct result destination, preventing

accidental replacement of an editable root configuration. Serial-based scan

selection and cleared caches avoid carrying applications from another target.

JSON5 is accepted for editing but .json device files are staged as strict JSON;

comments/formatting are normalized, not protocol values. Duplicate keys and

non-finite numbers are rejected rather than silently collapsed. AVB metadata

inspection now includes property descriptors, with offset and allocation bounds;

this does not establish a verified AVB signature or complete the downgrade port.

Debug rendering found scroll-state ID collisions in the two firmware lists.

Unique IDs repair state ownership rather than hiding the debug warnings; the

all-pages renderer regression checks that the diagnostic does not reappear.

The v0.2 release reports 45 tests and retains all hardware-validation limits.

## 0.3 — approved Guided Studio / Complete Root Management plan

The user explicitly authorized implementing the supplied five-milestone 1.0 plan.
Retain Rust/egui, executable name, existing CLI, data and historical packages.
Deliver foundations first; expose hardware-unverified status throughout rather
than equating a pinned release with a tested device combination. Provider-specific
capabilities remain distinct. Core and optional MCP are separate Cargo packages.
MCP cannot approve/execute; only the desktop confirmation grants approval.

Interrupted operation recovery is inspection/review, never automatic replay.
Critical cancellation and timeout avoid killing an active write. New APatch
scripts use stdin; prior key-bearing folders are preserved. Backups match exact
device/build/slot before restore planning; execution rechecks the Android build. Declared knowledge delta: README,
ARCHITECTURE, PARITY, IMPLEMENTATION, MCP, VALIDATION and project AGENTS/skill.
The remaining 0.4–1.0 work is not waived by packaging this staged release.

## 0.3.1 — Google catalog failure repair

User reported “Google firmware page returned no direct ZIP links”. Live unauthenticated
pages contained only an acknowledgement wall. Upstream runtime.py uses Devsite
acknowledgement cookies; live requests with the respective cookie restored both
tables. Add explicit caller consent and a persisted default-false UI checkbox with
terms links rather than treating the user's agreement as implicit. The CLI flag
allows end-to-end metadata verification through the same Rust loader. Knowledge
delta: README, ARCHITECTURE, PARITY, VALIDATION, this decision and port-validation
skill. Review trigger: Google wall/cookie identifiers, host/layout or terms changes.

## 0.3.2 — public Rust Edition repository and release

User requested repository cleanup, GitHub release and the PixelFlasher Rust Edition
name, and explicitly selected a public repository on contactjayclatty. Create
PixelFlasher-Rust-Edition, retain upstream attribution/license provenance and publish
a development prerelease. Move shared core files into their owning Cargo crate,
retain compatibility facade, CLI/executable/data names, and preserve older packages.
Add Windows CI, contributor/release instructions, dependency notices and versioned
portable/source allowlists. Knowledge delta: README, CHANGELOG, CONTRIBUTING, AGENTS,
core AGENTS, ARCHITECTURE, RELEASING, PARITY, VALIDATION and release-validation.json.
Publication includes source and checksums; it does not certify full 1.0 parity or
hardware workflows. User data, credentials, cached HTML and developer artifacts
are excluded from the public repository and release archives.
