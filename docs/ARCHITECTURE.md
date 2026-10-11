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

# Milestone 1A: OOXML / OPC package layer

Milestone 0 above remains the rendering baseline. The workspace now adds
`o2p-ooxml`; only `o2p-cli` gains a dependency on it. Neither `o2p-core` nor
`o2p-pdf` changes. The new entry path ends at inspection:

```text
PPTX file -> bounded ZIP reader -> OPC metadata / relationships -> package inventory
```

The future rendering pipeline remains OOXML package -> format parser ->
normalized model -> layout -> DisplayList -> PDF. The package crate has no
rendering dependencies or PDF commands. Slide XML, shapes, text, themes,
masters, layouts and drawings remain opaque part bytes.

## Public package contract

`OoxmlPackage::open(path, SecurityLimits)` and
`OoxmlPackage::from_bytes(bytes, SecurityLimits)` validate and own a package.
The package exposes immutable `PackagePart` values (name, resolved content type
and bytes), `ContentType` declarations, and relationships scoped to a source
part. `relationships(None)` returns `_rels/.rels`; `relationships(Some(name))`
returns that part's `.rels` or an empty slice when absent. An unknown source
part is an error. `[Content_Types].xml` is exposed through parsed declarations,
rather than as an ordinary part. Directory entries are not parts.

`Relationship` owns an ID, type URI and `RelationshipTarget`. An internal target
contains a canonical package-root-relative name; an external target contains an
opaque string. No API dereferences external targets. Relationships keep XML order;
part inventories are sorted lexicographically. Content type overrides take
precedence over case-insensitive extension defaults. Part names are case-sensitive.
Duplicate parts (including normalization aliases), duplicate relationship IDs,
duplicate defaults/overrides, missing content types, dangling overrides and
internal relationships, and orphan relationship parts are errors. Every `.rels`
part is validated, including parts outside the presentation's reachable graph.
Cycles are allowed: opening never recursively walks relationships.

Failures use the owned `PackageError` enum: I/O, ZIP, XML, path, metadata,
missing/duplicate parts, named security limits, unsupported features and non-PPTX.
No third-party error/type appears in the public API. Malformed or unsupported
metadata returns an error rather than being silently discarded. The reader does
not claim compatibility analysis of the opaque Office content inside the parts.

## Adapter and dependency decisions

ZIP indexing and metadata XML parsing live in private `zip` and `xml` modules.
This milestone was implemented and validated without network access. No cached
ZIP or XML parser crate was available, so these deliberately narrow adapters use
the already cached libraries below rather than fetching dependencies:

- `flate2` 1.1.10, `MIT OR Apache-2.0`: raw DEFLATE decoding, with defaults off
  and only `rust_backend` enabled (Rust `miniz_oxide`, no native zlib/service).
- `crc32fast` 1.5.2, `MIT OR Apache-2.0`: verify each member's CRC32 checksum.

Licenses were checked in the installed crate manifests. Both libraries were
already present in the Milestone 0 lockfile as transitive test dependencies;
this milestone introduces production dependency edges. There is no new network
or runtime service. The custom adapters are intentionally limited, not general
ZIP/XML implementations, and can be replaced without changing public types.

The ZIP adapter supports single-disk ZIP32, stored and DEFLATE members, comments,
empty directories, and data descriptors with/without signatures. It validates
central/local names, methods, flags, sizes, CRCs, record bounds and overlapping
members. Encrypted archives, ZIP64, split archives, other compression methods,
symlinks, alternate Unicode path extra fields and non-UTF-8 names fail explicitly.
Innocuous extra fields and ZIP comments are opaque and never executed.

The metadata adapter accepts XML 1.0 in UTF-8 (optional BOM) or UTF-16 with BOM,
default/prefixed namespaces, comments, empty or paired metadata elements, and
predefined/numeric entities. Metadata has exactly two element levels: `Types`
with `Default`/`Override`, or `Relationships` with `Relationship`. Nested metadata,
unknown elements/attributes, processing instructions, DTDs, entity declarations,
CDATA, unsupported encodings and non-ASCII XML names return errors. Arbitrary
slide XML is not passed through this limited adapter. Namespace bindings are
looked up without copying the root namespace table into each child.

## Path and resource security

There is no filesystem extraction. Names and targets are validated before
reading member payloads. `normalize_part_path` removes `.` segments and rejects
`..`, empty segments, absolute/drive/UNC paths, backslashes, control characters,
spaces and URI schemes/query/fragment syntax. Internal relationship resolution
allows `..` only when it remains within the package root, so ordinary
`../media/image1.png` works. Escaped names remain escaped; malformed escapes and
encoded dots, separators, controls, URI delimiters and percent signs are rejected
to avoid traversal or double decoding. Content-type override `PartName` has the
OPC-required leading `/`; that marker is removed before the same validation.
A leading `/` on a ZIP name or relationship target is rejected.

Default `SecurityLimits` are:

| Limit | Default |
| --- | --- |
| Input bytes | 64 MiB |
| ZIP entries (including directories) | 10,000 |
| Total uncompressed bytes (including metadata) | 256 MiB |
| Individual part bytes | 64 MiB |
| Uncompressed/compressed ratio per member | 1,000 |
| Metadata XML bytes per stream | 4 MiB |
| Relationships across the package | 100,000 |
| ZIP / resolved internal path bytes | 1,024 |

Callers can override every limit. Declared sizes and counts are checked before
payload decoding. DEFLATE output is read in fixed-size chunks and capped at the
declared size plus one byte; a stream must terminate exactly, consume its full
compressed range, match its declared size, and pass CRC. Actual aggregate output
is checked again. XML is nonrecursive and cannot load DTDs or expand user-defined
entities. Input bytes, decompressed parts and bounded temporary metadata structures
are held in memory; limits are resource bounds, not an exact heap usage budget.
A path-length limit also bounds repeated normalized relationship targets.
Streaming parts and fuzzing infrastructure are deferred.

## PPTX inspection and validation

PPTX detection requires exactly one package-level `officeDocument` relationship
with an internal target of content type
`application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml`.
Both Transitional and Strict office-document relationship URIs are recognized.
The file extension and a hardcoded `ppt/presentation.xml` location are not used
for detection. Macro-enabled/template formats are not identified as PPTX.

`inspect_pptx()` returns `PptxInspection` with the main part, all parts declaring
the slide content type, and non-relationship parts in the main part's sibling `media/` directory
(normally `ppt/media/`). Counts and inventories concern package parts, including
orphan slides/media; they are not presentation playback order or a count of slide
references. This avoids parsing `presentation.xml` in this milestone. Parts such
as `docProps/thumbnail.jpeg` are not counted as presentation media.

```sh
cargo run --offline -p o2p-cli -- inspect fixtures/pptx/one-image.pptx
```

`o2p inspect` prints format, counts and names. Invalid input returns a nonzero
status and an error on stderr, with no partial inventory or output document.
The existing `demo` command is unchanged. Synthetic fixtures and their Rust
regenerator live under `fixtures/pptx/` and `o2p-ooxml/examples/`; tests cover
metadata, paths, detection, inventories, missing targets, ZIP corruption,
resource limits, descriptors and hostile compressed streams, plus malformed
input mutation/truncation cases and CLI behavior. This is not a formal security
proof or a substitute for a future sustained fuzzing campaign.
