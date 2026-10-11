# Implementation roadmap

## Milestone 0 — DisplayList to PDF (complete)

Baseline commit: `9db5d54`.

- `o2p-core`: format-independent geometry and positioned drawing operations.
- `o2p-pdf`: validated DisplayList to deterministic PDF bytes.
- `o2p-cli`: `o2p demo <output.pdf>`.
- Unit, PDF integration and CLI tests.

This milestone remains intact; the package layer does not change the drawing
model or PDF backend.

## Milestone 1A — OOXML / OPC package layer (implemented)

- Add `o2p-ooxml` to the workspace, with private ZIP/XML adapters and owned public
  package, part, relationship, content-type, error and security-limit types.
- Open bounded ZIP32 packages with stored/DEFLATE entries and validate their
  structure, names, sizes, stream termination and checksums.
- Read `[Content_Types].xml`, package relationships and every part-level `.rels`.
- Resolve relative internal targets inside the package root. Preserve external
  targets as opaque strings and never fetch them.
- Reject traversal, absolute paths, duplicate metadata, malformed XML,
  orphan/dangling relationships and excessive resource consumption.
- Detect PPTX through package metadata; list slide parts and the main part's
  sibling media directory, with counts and deterministic names.
- Add `o2p inspect <file.pptx>` and deterministic synthetic fixtures/tests.
- Resolve and validate dependencies offline using cached `flate2` and
  `crc32fast` (`MIT OR Apache-2.0`).

Current limits: single-disk ZIP32, stored/DEFLATE, UTF-8 ZIP names and flat OPC
metadata XML 1.0 in UTF-8/UTF-16. Unsupported constructs return errors. Parts
remain bounded in-memory bytes. Inspection reports a package inventory, not
presentation order; slide content and presentation XML remain unparsed.
See [ARCHITECTURE.md](ARCHITECTURE.md) for exact contracts and defaults.

## Milestone 1B — PPTX parsing and normalized model (next, not implemented)

Use the package abstraction to parse presentation ordering, slide dimensions
and the agreed basic slide content. Add a PPTX parser/model without allowing
format parsing to emit PDF commands. Unsupported Office content must produce
structured warnings. Keep chart, SmartArt, WordArt and complex-effect support
outside the initial scope.

## Milestone 1C — PPTX layout to DisplayList (planned)

Lay out the normalized slide model into the common DisplayList and integrate
with the existing PDF backend. Add rendering regression fixtures for text,
images, rectangles, ellipses, lines, fills and borders. Extend shared primitives
only as needed for demonstrated rendering behaviors.

Later format support, richer typography, Flutter integration and advanced Office
features remain future milestones. DOCX and XLSX are not part of current work.

## Validation

```sh
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

Use `CARGO_NET_OFFLINE=true` (or `--offline`) when running from the populated
local dependency cache. Package inspection and rendering never need network
access or external runtime executables.
