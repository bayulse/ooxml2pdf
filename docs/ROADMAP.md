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

## Milestone 1B — PPTX metadata to blank PDF pages (complete)

- Add `o2p-pptx` for PresentationML semantics, separate from OPC mechanics.
- Parse the detected main presentation part, real EMU slide size and ordered
  slide ID list, supporting Transitional/Strict namespaces.
- Resolve internal slide relationships in presentation order; never infer
  ordering from filenames, sorted inventories or relationship-file order.
- Own normalized `Presentation`, `SlideRef`, `SlideSize`, errors and warnings;
  expose no XML implementation types.
- Produce one format-independent blank page per slide reference, using points,
  then pass the DisplayList to the unchanged PDF backend.
- Add `o2p convert <input.pptx> -o <output.pdf>`; keep `inspect` and `demo` working.
- Leave all slide contents opaque and return structured warnings for omitted
  rendering and unsupported presentation metadata.
- Add bounded XML parsing with a private `roxmltree` adapter
  (`MIT OR Apache-2.0`), DTD rejection, node/byte/slide limits and a depth
  preflight before recursive tree construction.
- Add deterministic standard/widescreen/reordered/invalid-size/deep-nesting
  fixtures, parser tests and independent PDF/CLI integration tests.

The first end-to-end pipeline is now PPTX -> OPC package -> presentation model
-> blank core Pages / DisplayList -> existing PDF renderer -> PDF. Slide content
is intentionally not rendered. Empty slide-reference lists are rejected. See
[ARCHITECTURE.md](ARCHITECTURE.md) for contracts, units, warnings and limits.

## Milestone 1C — Slide backgrounds and rectangles (complete)

- Add content-aware PPTX parsing through the existing bounded XML adapter;
  preserve the metadata-only and blank-page APIs from Milestone 1B.
- Parse referenced slide parts in presentation order into owned `Slide`,
  `Shape`, `Fill`, and `Stroke` values.
- Render explicit solid sRGB slide backgrounds and axis-aligned rectangle
  geometry, solid/no fill, and simple solid outlines.
- Centralize EMU-to-point conversion for slide sizes, signed positions,
  dimensions and outline widths.
- Convert the normalized model to format-independent core operations. Add
  only `StrokeRectOp` / `DrawOp::StrokeRect` to the existing core/PDF contract.
- Report unsupported colors, transforms, styles, inherited properties and
  other content through slide-scoped structured warnings.
- Switch `o2p convert` to the content-aware path; preserve `inspect`, `demo`,
  blank output, CLI arguments, deterministic output and failure handling.
- Add deterministic synthetic backgrounds/rectangle/order/style fixtures,
  parser/security tests, independent PDF operator tests and CLI conversion tests.
- Reuse all existing dependencies; package/OPC behavior remains unchanged.

Current content scope is deliberately limited to backgrounds and rectangles.
Text, images, ellipses, lines, groups, themes, masters, layouts, gradients,
transparency, charts, SmartArt and animations are deferred. Unresolved colors
and styles are warned about rather than guessed. See
[ARCHITECTURE.md](ARCHITECTURE.md) for the precise behavior and defaults.

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
