# Attribution and license provenance

Upstream: PixelFlasher, Copyright (C) 2026 Badabing2005 and contributors.
Repository: https://github.com/badabing2005/PixelFlasher
Reference revision: 081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54.

The Rust application ports documented workflows and protocol behavior from that
revision. Its implementation does not invoke or embed the upstream Python app.
Public certificate fixtures used for tests were copied from the upstream
PixelFlasher.crt and ca_badabing2005.crt files; no private signing key is included.

Upstream has differing license declarations: its root LICENSE contains GPLv3,
while Python source headers declare AGPL-3.0-or-later. The source-port package
uses AGPL-3.0-or-later and includes the full text as LICENSE. The unchanged upstream
root text is preserved as LICENSE.upstream. Resolve upstream's declaration
discrepancy with its maintainers before making redistribution assumptions.

Rust dependencies retain their own licenses; Cargo.lock records the exact
dependency set. The application is an independent development port, not a Google
product or a claim of upstream-maintainer endorsement. Pixel is a Google trademark.

