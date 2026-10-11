# Milestone 0: Common DisplayList to PDF

Milestone 0 proves the last stage of the intended pipeline:

```text
hardcoded DisplayList -> o2p-pdf -> PDF bytes -> CLI file output
```

The long-term pipeline remains OOXML package -> format parser -> normalized
document model -> layout -> common DisplayList -> PDF. The parser and layout
stages are deliberately absent from this milestone. The PPTX priority in
AGENTS.md applies to later work; Milestone 0 establishes the shared output path.

## Workspace and dependency direction

The root is a virtual Cargo workspace with exactly three members:

| Crate | Responsibility | Production dependencies |
| --- | --- | --- |
| `o2p-core` | Geometry, color, ordered pages and drawing operations | None |
| `o2p-pdf` | Validate and render a DisplayList to PDF bytes | `o2p-core`, `pdf-writer` |
| `o2p-cli` | Hardcoded demo and file output; binary named `o2p` | `o2p-core`, `o2p-pdf` |

The initial unused root crate is replaced by this workspace. No facade crate,
backend trait, plugin registry, parser adapters, or configuration framework is
needed yet. All workspace crates forbid unsafe code. The workspace uses Rust
2024 and declares Rust 1.85 as its minimum version; validation is currently on
Rust 1.93.1, so minimum-version compatibility is not yet independently tested.

`pdf-writer` 0.14 is the only direct third-party production dependency. It is a
small, low-level pure-Rust writer, dual-licensed MIT OR Apache-2.0. It lets the
backend explicitly control page objects, resources and content streams without
pulling in Office parsing, layout, font discovery or image decoding. Although
PRODUCT_SPEC.md mentions `printpdf` as a candidate, the smaller writer is enough
for the already-positioned commands of this milestone. PDF objects, references,
fonts and content-stream types remain private to `o2p-pdf`. The lockfile pins
the resolved dependency graph.

Tests use `lopdf` 0.38 (MIT) with default features disabled to independently
read generated PDFs and inspect their text, page tree and drawing operators.
It is a development dependency only. Dependency resolution may need network
access at build time; rendering never needs network access, a system font, a
browser, LibreOffice, JavaScript, or an external executable.

## DisplayList contract

Public types in `o2p-core` are small owned structs and enums:

- `Point { x, y }`, `Size { width, height }`, and `Rect { origin, size }` use
  `f32` points (1/72 inch). The page origin is top-left, x grows rightward,
  and y grows downward. Negative positions are valid.
- `Color` is opaque sRGB with three byte channels. Alpha and other color
  spaces are deferred.
- `DisplayList` owns ordered `Page` values. Each page owns its dimensions and
  a `Vec<DrawOp>` painted in insertion order. There is no implicit background
  operation.
- `DrawOp::Text(TextOp)` holds owned text, a baseline origin, logical font,
  font size and color. `Font::SansSerif` is the only current logical face.
- `DrawOp::Rect(RectOp)` fills an axis-aligned rectangle with a color.
- `DrawOp::Line(LineOp)` draws a solid, positive-width line between two points.

The model has no Office or PDF types and performs no layout, shaping or I/O.
Geometry fields are public for straightforward construction; the rendering
boundary validates them. Text remains Unicode in the model so backend limits
do not define the model's string representation.

Images are omitted: an image handle, asset store and decoding policy are not
needed to prove this pipeline. Introducing an unusable operation would add a
contract without demonstrating its behavior. Font embedding, glyph runs,
rectangle borders, paths, transforms, clipping operations and transparency are
also deferred until a concrete renderer needs them.

## PDF backend

The public entry point is:

```rust
pub fn render(display_list: &DisplayList) -> Result<Vec<u8>, PdfError>;
```

The backend writes PDF 1.7 with a catalog, page tree, a shared standard
Helvetica font resource and one uncompressed content stream per page. It
supports one or multiple pages, including pages without drawing operations.
Each drawing operation saves and restores graphics state. Text uses an upright
text matrix at `(x, page_height - baseline_y)`. Filled rectangles use
`(x, page_height - y - height)`; both line endpoints use `page_height - y`.
Page bounds provide ordinary PDF page clipping.

The logical sans-serif font maps to regular PDF standard Helvetica. Printable
ASCII (U+0020 through U+007E) is supported, including PDF string delimiters and
backslashes. Non-ASCII characters and controls, including line breaks and tabs,
return `UnsupportedText`; there is no silent replacement, fallback or dropped
content. Font embedding, complex shaping and cross-viewer font fidelity are
outside this proof of concept. Callers position each single-line run themselves.

Before any PDF generation, the entire document is validated:

- At least one page is required, and PDF object numbering must fit `i32`.
- Page dimensions, text size and line width must be finite and positive.
- Coordinates must be finite; rectangle dimensions must be finite and
  nonnegative. Zero-sized rectangles are allowed.
- Coordinate conversion results must be finite, including arithmetic overflow
  checks for finite input values.
- Every text character must be supported.

`PdfError` is a library-owned standard Rust error enum. Geometry and text errors
carry zero-based page and operation indices; page-size errors have no operation
index. The backend returns no partially rendered document on validation failure.
The library does not write files; the CLI reports file I/O failures separately.

Object numbering, resource names, painting order and serialization are stable.
No timestamps, document IDs, random values or system-dependent resources are
included. Identical input yields byte-identical output with the same backend
version. Byte stability across future dependency upgrades is not promised.
The implementation holds the DisplayList and output bytes in memory; streaming
and resource limits beyond the object-count bound are future work.

## Demo and validation

```sh
cargo run -p o2p-cli -- demo target/demo.pdf
cargo fmt
cargo clippy --all-targets --all-features
cargo test --workspace
```

The demo paints a pale blue rectangle, two ASCII text runs and a blue line on
one approximately A4 page (595 x 842 points). The output path is required;
`--help` prints usage. An existing destination file is overwritten. Parents
must exist, and write failures return a nonzero exit status. The CLI uses
standard-library argument parsing and accepts OS-native paths.

Core tests cover color representation and ordering. Backend integration tests
independently parse the generated document, verify page dimensions, font
resources, text escaping, geometry, color, graphics-state isolation, paint order,
multiple pages and byte determinism. Negative tests exercise empty documents,
invalid geometry, conversion overflow and unsupported text. CLI integration tests
exercise the executable, deterministic files, usage, help and write failures.

No DOCX, XLSX or PPTX parsing, Flutter integration, OCR, or advanced Office
features are implemented.
