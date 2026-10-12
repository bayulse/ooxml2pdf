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

At completion of 1C, content scope was limited to backgrounds and rectangles.
Milestone 1D below adds ellipses and straight lines. Text, images, groups, themes,
masters, layouts, gradients, transparency, charts, SmartArt and animations remain
deferred. Unresolved colors
and styles are warned about rather than guessed. See
[ARCHITECTURE.md](ARCHITECTURE.md) for the precise behavior and defaults.

## Milestone 1D — Ellipses and straight lines (complete)

- Extend the existing slide parser with `prstGeom` ellipse and line geometry;
  accept straight lines in both `sp` and `cxnSp` without implementing routing.
- Add only normalized `Shape::Ellipse` (bounding rectangle, fill, stroke) and
  `Shape::Line` (start, end, stroke); retain the existing rectangle variant.
- Reuse centralized EMU conversion, sRGB colors, solid/no fill, and outline
  parsing, including its width defaults and structured unsupported-style warnings.
- Convert xfrm offset/extent to line endpoints and apply horizontal/vertical
  flips; support horizontal, vertical and diagonal lines, signed positions and
  zero extents. Nonzero rotations remain unsupported and warned about.
- Reuse core `LineOp`. Add generic `EllipseOp` and `StrokeEllipseOp`, with
  validated PDF rendering through four closed cubic quarter-arcs.
- Preserve XML paint order across rectangles, ellipses and lines, with each
  shape's fill before its outline. Preserve metadata-only and blank-page APIs,
  inspection, rectangle behavior and deterministic PDF output.
- Add eight small reproducible fixtures, normalized geometry/paint/order tests,
  independent PDF operator/validation tests and a mixed-shapes CLI conversion test.
- Introduce no dependencies and no OPC changes. Text, images, groups, arrows,
  routed connectors, themes and inherited styles remain unsupported.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/mixed-shapes.pptx -o target/mixed-shapes.pdf
```

Later format support, richer typography, Flutter integration and advanced Office
features remain future milestones. DOCX and XLSX are not part of current work.

## Milestone 1E — Embedded raster pictures (complete)

- Parse direct `p:pic` embedded PNG/JPEG references and explicit transforms through
  bounded slide XML and existing package relationship/part APIs.
- Add shared encoded resources and destination rectangles to the generic core;
  normalize pictures in the existing ordered slide shape vector.
- Copy each referenced media part once, sharing it across pictures/slides/models;
  decode each resource once inside PDF and release pixel buffers per image.
- Render RGB image XObjects with normal PNG alpha masks, preserving EMU placement,
  size, orientation and z-order among backgrounds, rectangles, ellipses and lines.
- Bound dimensions, pixels and decoder allocation; return owned errors for bad
  references or corrupt media, preserving existing CLI output on failure.
- Warn and omit unsupported formats, crops, tiling, destination stretch rectangles,
  image effects, flips and rotations.
- Add six synthetic fixtures, parser/ownership/PDF/security/overlap/CLI tests and
  retain inspection and completed milestone regression coverage.
- Add private PNG/JPEG-only `image` 0.25.8 (MIT OR Apache-2.0, Rust 1.85) and reuse
  pure-Rust `flate2`; no external runtime processes/services are added.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/image-with-shapes.pptx -o target/image-with-shapes.pdf
```

PPTX text, SVG, advanced cropping/effects, groups, themes, masters, layouts,
gradients, charts, SmartArt and animation remain deferred. DOCX, XLSX and Flutter
remain outside this milestone. See ARCHITECTURE.md for exact limits and decisions.

## Milestone 1F — Basic text boxes (complete)

- Parse common shape text bodies into owned text boxes, ordered paragraphs and
  literal Unicode runs with direct size, bold, italic and sRGB color.
- Preserve shape transforms, existing fills/outlines, and text/image/vector z-order.
- Lay out explicit paragraph/line breaks with fixed spacing and left/center/right
  horizontal alignment; use direct insets/local paragraph defaults when present.
- Document and warn about unresolved inheritance (18pt regular black sans-serif),
  unsupported text features and box overflow. No wrapping or autofit is added.
- Extend generic core font choices and fixed metrics without changing TextOp fields.
- Extend the private PDF adapter to WinAnsi Unicode and four standard Helvetica
  faces. The CLI reporting API replaces missing glyphs with `?` and warns;
  “Résumé” renders directly, while “日本語” deterministically becomes “???”.
- Preserve the strict PDF render API and previous inspection, metadata, blank-page,
  shape and image behavior. No dependency changes or runtime services.
- Add eight reproducible fixtures plus parser, layout, PDF/Unicode/font, z-order
  and end-to-end CLI tests; visually inspect text with vector/raster content.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/text-with-image.pptx -o target/text-with-image.pdf
```

Later typography work (Milestone 1G and beyond) may add better wrapping, shaping,
font coverage/resolution and inheritance. Font discovery, substitution tables,
embedded/theme fonts, master/layout inheritance, autofit, vertical/rotated text,
bullets/numbering, spacing/kerning controls, superscripts/underline/strikes, complex
effects, groups, charts and SmartArt remain deferred. DOCX, XLSX and Flutter remain
outside the implemented scope. See ARCHITECTURE.md for precise contracts.

## Milestone 1G-A — Generic fonts, Unicode shaping, and embedding (complete)

- Add `o2p-fonts` with explicit byte/file/directory registration, generic requests,
  family/style matching, configured aliases/fallbacks, glyph availability and
  structured substitution/missing-font/missing-glyph diagnostics.
- Adapt maintained HarfRust plus ttf-parser privately; segment Unicode scripts,
  bidi directions and graphemes without desktop system-font discovery.
- Preserve direct PPTX latin typefaces in the normalized model; keep theme,
  master/layout and east-Asian/complex-script typeface selection unsupported.
- Add generic owned shaped glyph/source/font resources to dependency-free core;
  measure the existing unwrapped layout with shaped advances.
- Embed complete static TrueType programs using Type0/CIDFontType2, explicit
  CIDToGIDMap, UTF-16BE ToUnicode and logical-source ActualText. Keep actual PDF
  text and legacy TextOp behavior. Font subsetting remains deferred.
- Bound font/shaping/PDF mapping workloads; reject unsupported font technologies
  and restricted embedding rights with owned errors.
- Add small licensed test fonts, 日本語 and other Unicode fixtures, independent
  extraction tests and all existing Latin/shape/image/text paint-order regressions.
- Add optional CLI `--font`, `--font-dir` and `--fallback-family` configuration;
  no implicit bundled fonts or platform defaults.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/unicode-fonts.pptx -o target/unicode-fonts.pdf --font-dir fixtures/fonts --fallback-family "O2P Test Sans" --fallback-family "O2P Test Japanese" --fallback-family "O2P Test Arabic"
```

See [FONTS.md](FONTS.md) for exact contracts, licenses and reader-dependent RTL
extraction limits. Full PowerPoint typography, inherited/theme/OOXML embedded
fonts, wrapping/reflow/autofit, bullets, vertical/rotated text, cross-style bidi,
subsetting, variable/color/CFF fonts and mobile integration remain later work.

## Validation

```sh
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

Use `CARGO_NET_OFFLINE=true` (or `--offline`) when running from the populated
local dependency cache. Package inspection and rendering never need network
access or external runtime executables.
