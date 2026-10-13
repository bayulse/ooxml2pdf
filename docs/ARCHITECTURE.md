This document records completed milestones chronologically. Milestone 1G-B
describes the current configured-font layout path and the final Milestone 1H
section adds individual element transforms. Milestone 1I-A below adds explicit
theme resolution; Milestone 1I-B adds structural inheritance; 1I-C adds DrawingML format styles. Earlier standard-font descriptions
remain the compatibility-path history.

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

At Milestone 0, the root became a virtual Cargo workspace with three members:

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

# Milestone 1B: PPTX metadata to blank PDF pages

Milestones 0 and 1A remain unchanged. The workspace adds `o2p-pptx`, and the CLI
orchestrates the first complete input-to-output path:

```text
real PPTX -> o2p-ooxml package -> o2p-pptx metadata / normalized Presentation
          -> o2p-core DisplayList with blank Pages -> o2p-pdf -> PDF bytes
```

| Crate | Responsibility | Production dependencies |
| --- | --- | --- |
| `o2p-ooxml` | ZIP / OPC mechanics, parts, content types and resolved relationships | `flate2`, `crc32fast` |
| `o2p-pptx` | PresentationML size, slide references, ordered model and blank pages | `o2p-ooxml`, `o2p-core`, `roxmltree` |
| `o2p-core` | Format-independent geometry, pages and drawing operations | None |
| `o2p-pdf` | Validate DisplayList and produce PDF bytes | `o2p-core`, `pdf-writer` |
| `o2p-cli` | Open input, parse, convert, render, report warnings and write output | The four workspace libraries |

The package inspection API still identifies PPTX by its package-level
`officeDocument` relationship and main-part content type. `Presentation::parse`
uses that main part (normally `ppt/presentation.xml`), without assuming its
location. No package-layer behavior changes are required. Inspection continues
to report sorted inventories, including orphan parts; conversion uses ordered
slide references instead.

## Normalized model and ordering

`Presentation::parse(&OoxmlPackage, ParseLimits)` returns an owned model with
private fields and read-only getters:

- `SlideSize` preserves integer width/height in EMUs. `to_points()` returns
  the common `Size`. Conversion divides by 12,700 in `f64`, then rounds once
  to the core's `f32` points (914,400 EMUs per inch / 72 points per inch).
- `SlideRef` owns its numeric slide ID and canonical resolved part name.
- `Presentation` owns the global size, ordered references and structured
  `PptxWarning` values. It outlives the package and XML parser.

The namespace-aware parser supports Transitional and Strict PresentationML and
office-document relationship URIs. It reads direct `sldSz` and `sldIdLst`
children of `presentation`, then resolves each namespaced `r:id` against the
main part's relationships in **slide-list order**. Relationship-file order,
numeric slide IDs, filename numbering and sorted inventories do not determine
page order. Targets must be internal relationships of type `slide`, pointing
to parts with the PresentationML slide content type. External targets are
never fetched. Unreferenced slides do not produce pages.

Missing/duplicate size or slide lists, missing required attributes, invalid
integers, duplicate numeric IDs or relationship references, nested/text content
in supported metadata, and bad slide relationships return owned `PptxError`
values. Slide dimensions must lie within the OOXML slide-size range of
914,400–51,206,400 EMUs, and slide IDs within 256–2,147,483,647. The size preset
`type` does not override `cx`/`cy`. This is a supported metadata subset, not full
OOXML schema validation. Presentations without slide references are rejected
explicitly, because the existing PDF renderer requires at least one page.

`to_blank_display_list()` iterates the normalized slide vector and produces
exactly one `Page { size, ops: vec![] }` per reference. Page index corresponds
to the same slide-reference index; hidden slides are included because slide
content is not examined. With identical blank pages, slide identity cannot be
distinguished visually in the PDF. Ordering is verified in the normalized model
and CLI warning sequence; the backend's existing page-order contract is used.
No PPTX type, XML tree, or format-specific command enters `o2p-pdf`.

## Opaque content, warnings and parser security

No slide bytes are interpreted. Text, images, shapes, themes, masters, layouts,
charts, SmartArt, animations and other slide features remain opaque. Every
referenced slide yields `SlideContentNotRendered { slide_id, part }`, including
empty slides. Other presentation elements/subtrees and unrecognized attributes
yield `UnsupportedPresentationMetadata { part, name }`. Their contents are not
interpreted, and warnings do not claim a content compatibility scan. Diagnostic
namespace/local names are clipped to 160 characters each to avoid copying large
shared namespace bindings into every warning. Malformed presentation XML is an
error, even in an unsupported subtree; slide XML is not parsed or validated.

`roxmltree` 0.21.1 is a maintained, pure-Rust read-only XML parser, licensed
`MIT OR Apache-2.0`; its installed manifest declares Rust 1.60 compatibility.
It provides namespace resolution, grammar validation, predefined/numeric
entities and a node limit. It is isolated in `o2p-pptx`'s private XML adapter;
no parser types/errors enter the public model/API. Its default features add
no runtime services. ZIP/OPC parsing remains in its existing adapters.

The adapter accepts XML 1.0 in UTF-8 (optional BOM), or UTF-16 LE/BE with BOM;
declarations must agree with the supported encoding. All DTDs, including empty
ones, are rejected, and no entity resolver is supplied. The XML tree builder
uses recursion: an iterative lexical preflight checks nesting **before** tree
parsing. Both self-closing and paired tags count toward depth; comments,
processing instructions, CDATA and quoted tag attributes are skipped correctly.
The tree parser then validates XML grammar and namespace bindings.

Default `ParseLimits` are independent of package limits:

| Limit | Default |
| --- | --- |
| Raw and decoded presentation XML bytes | 4 MiB |
| XML nodes | 100,000 |
| XML element depth | 64 |
| Referenced slides / generated pages | 10,000 |

The depth option can lower the adapter's hard ceiling of 64, but cannot raise
it; this protects recursive parser stack usage even with caller-supplied limits.
Other limits are configurable. No recursive package traversal is introduced.
Parts, the bounded XML tree, normalized model, DisplayList and PDF remain
in-memory; these bounds are not an exact heap-usage budget.

## CLI and validation

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/three-slides-16x9.pptx -o target/three-slides.pdf
```

This writes three blank 960 × 540 point pages (13⅓ × 7½ inches). Standard 4:3
fixtures produce 720 × 540 point pages. Warnings go to stderr; success reports
the output path and blank page count on stdout. Input/parsing/rendering failures
return nonzero before touching output. As with `demo`, an existing destination
is overwritten after successful parsing/rendering, its parents must exist,
and write errors return nonzero. OS-native path arguments are preserved.
`demo` and `inspect` retain their behavior.

The new deterministic fixture generator reuses Milestone 1A's test-only ZIP
builder and leaves its fixtures intact. Tests cover EMUs, both sizes, namespace
handling, UTF-8/UTF-16, relationship order/types, relocated main parts, orphan
slides, opaque slide bytes, warnings, malformed metadata/XML, hostile nesting
and configurable limits. Independently parsed PDFs verify exact page counts,
MediaBoxes and empty content streams. CLI tests cover conversion, warnings in
presentation order, deterministic overwrites, help, errors and preservation of
existing output on input failure. A deeply nested presentation is a regression
fixture for rejection before recursive tree parsing.

# Milestone 1C: Slide backgrounds and rectangles

The content-aware pipeline is now:

```text
PPTX -> o2p-ooxml package -> o2p-pptx metadata and slide XML
     -> normalized Presentation / Slide / Shape / Fill / Stroke
     -> o2p-core DisplayList -> o2p-pdf -> PDF bytes
```

OPC reading and inspection are unchanged. `o2p-pptx` owns PresentationML and
DrawingML semantics, unit conversion, and the normalized-model-to-DisplayList
adapter. The core and PDF crates contain no PPTX concepts. No new dependency
is introduced; the existing private `roxmltree` adapter is reused for slides.

## APIs and normalized content

`Presentation::parse` and `to_blank_display_list` retain Milestone 1B behavior,
including opaque slide bytes and `SlideContentNotRendered` warnings.
`Presentation::parse_with_content` first resolves metadata through that same
path, then parses each referenced slide part in presentation order. It replaces
the blanket slide warnings with specific content warnings. Orphan slide parts
remain opaque. Malformed referenced slide XML fails conversion before output
is written. `slide_contents()` returns one owned `Slide` per ordered reference;
it is empty for metadata-only models. No XML parser types escape the adapter.

Each slide has an optional background and ordered `Shape::Rectangle` values.
Rectangle geometry uses core `Rect` in points. `Fill::Solid(Color)` represents
an opaque resolved sRGB color, and `Fill::None` an explicit `noFill`. A shape's
`Option<Fill>` distinguishes absent/unresolved paint from explicit no fill.
`Stroke { color, width }` represents an optional resolved solid outline, with
width in points. Content model values use `PartialEq` because geometry is `f32`;
`Presentation` consequently no longer implements `Eq`.

`to_display_list()` paints a supported background as a full-page filled
rectangle, then each shape's fill and outline in XML order. Metadata-only
models produce blank pages. `to_blank_display_list()` remains explicitly blank
even when called on a content-aware model. No fill operation is emitted for
explicit `noFill` or unresolved fill; a resolved outline still paints. Shapes
with no paint retain their normalized geometry but produce no drawing operations.

## Supported semantics, units and warnings

The parser supports Transitional and Strict PresentationML/DrawingML namespaces,
independently of namespace prefixes. It reads `cSld/bg/bgPr` backgrounds and
direct `spTree/sp` rectangle shapes with explicit `spPr`, `a:prstGeom prst="rect"`,
and `a:xfrm` offset/extent. Structural nonvisual metadata is not rendered.
An empty root `grpSpPr`, or an all-zero root transform commonly emitted by
Office writers, is accepted as scaffolding. Actual root group transforms omit
the shape tree with a warning; nested groups are unsupported.

All geometry uses the single `emu_to_points(impl Into<i64>)` helper: divide
by 12,700 in `f64`, then round once to `f32`. Existing `u32` slide sizes still
work; signed positions, dimensions and line widths use the same helper. Shape
offsets are bounded to DrawingML's coordinate range
−27,273,042,329,600–27,273,042,316,900 EMUs; extents are nonnegative up to the
positive upper bound. Negative positions and zero extents remain valid. Explicit
outline widths must be 0–20,116,800 EMUs; zero produces no stroke, avoiding an
accidental PDF hairline. For an explicit solid outline with omitted width,
the parser uses the [documented 0.75-point DrawingML base default](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/c0c046ec-a61d-405d-88fe-74d8487a37d7)
and warns that inherited width is unresolved. A missing outline is unresolved
and warned about; explicit line `noFill` requires no width default.

Colors require a six-digit hexadecimal `a:srgbClr/@val` (either case).
Scheme/theme, system and other color representations emit
`UnsupportedSlideContent { slide_id, part, name }` and omit the affected paint.
Color transforms, including alpha, tint and shade, also warn and omit paint.
The parser never substitutes a scheme color's name or a system fallback color.
Gradients, pattern/image/group fills and background style references are opaque
and warned about. Missing explicit shape paint warns about unresolved inheritance.

Only simple centered solid outlines are supported. Explicit `prstDash="solid"`
and default-valued `cap="flat"`, `cmpd="sng"`, `algn="ctr"` are accepted.
Other dashes, compound lines, joins, ends or alignment/cap styles warn and omit
the outline while preserving an independently supported fill. Rotation and
flips omit the rectangle with warnings; zero rotation and false flips work.
Nonrectangular/custom/inherited geometry, adjusted presets and placeholders
are omitted with warnings. Text bodies, pictures, graphic frames, groups,
effects, style references, color-map overrides, extensions and other content
are not rendered and receive warnings at their opaque subtree boundary.
Slide relationships (including layouts, images and external links) are warned
about and never followed. Themes, masters and layouts are not implemented.

Unsupported element/attribute warnings use bounded diagnostic names. Malformed
supported values, missing required geometry, conflicting fill declarations,
duplicate supported properties and invalid slide roots return the owned
`InvalidSlide` error. Malformed XML still returns `InvalidXml`. This remains a
supported subset rather than complete OOXML schema validation.

The same byte, decoded-byte, node and depth limits apply **per XML part**;
slide count remains bounded by the presentation parser. The adapter keeps its
64-element hard depth ceiling, DTD rejection, encoding checks and lexical
preflight before recursive tree construction. Slides are parsed one at a time;
XML trees are released after each normalized slide is built. Package limits bound
aggregate input; these limits are not an exact heap-usage budget.

## Minimal core/PDF integration and CLI

The existing filled `RectOp` remains unchanged. The sole new drawing primitive is
`StrokeRectOp { rect, color, width }` / `DrawOp::StrokeRect`, an axis-aligned closed
outline centered on its rectangle edges. PDF rendering validates its geometry
and positive finite width, isolates graphics state, converts top-left coordinates
using the existing convention, and emits a closed rectangle stroke with no fill.
The PDF backend receives only the DisplayList and never interprets PPTX XML.

`o2p convert` now calls `parse_with_content` and `to_display_list`; stdout reports
the page count without claiming all pages are blank. Empty slides still produce
blank pages. Warnings remain on stderr; argument parsing, native paths, inspection,
demo, deterministic output, overwrite and failure behavior remain intact.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/filled-rectangle.pptx -o target/filled-rectangle.pdf
```

The required six synthetic fixtures cover backgrounds, basic/nontrivial
rectangles, fills, outlines, no fill and overlapping shapes. Additional fixtures
verify visibly distinct reordered pages and unsupported scheme/dash styles.
Tests independently decode PDF operators to verify geometry, top-left conversion,
paint order, colors and width. Parser tests cover namespace/encoding variants,
unsupported features, malformed values and per-slide security limits. CLI tests
verify visible filled-rectangle commands, warnings, blank-page preservation,
determinism and failure behavior. Existing metadata/package/demo tests remain.

# Milestone 1D: Ellipses and straight lines

The 1C content pipeline is retained: PPTX XML -> normalized PPTX shapes ->
format-independent DisplayList -> PDF. OPC mechanics, metadata-only parsing,
blank-page conversion and rectangle rendering retain their existing behavior.
No dependency is added. `o2p-pptx` alone interprets presets, transforms and styles;
`o2p-pdf` receives only generic geometry and paint.

## Minimal model and DrawingML support

The existing `Shape::Rectangle { rect, fill, stroke }` remains unchanged. Two
variants extend the normalized model:

- `Shape::Ellipse { bounds, fill, stroke }` uses a core `Rect` bounding box,
  `Option<Fill>` and `Option<Stroke>`, just as rectangles do. The ellipse is
  inscribed in its box, rather than drawn as a rectangle.
- `Shape::Line { start, end, stroke }` stores core points and an optional
  resolved solid stroke. An open line has no interior fill.

Direct `spTree/sp` shapes accept `prstGeom` values `rect`, `ellipse` and `line`.
Direct `spTree/cxnSp` accepts only `line`; bent/curved connectors are warned
about and omitted. Nonvisual connector attachment children are warned about;
attachment relationships are never resolved or used to reroute the explicit
line. Shape XML order is preserved in the same normalized vector, regardless of
whether an element is `sp` or `cxnSp`. Both Strict and Transitional namespaces
work. Geometry still requires an explicit offset and nonnegative extent.

For a line, offset `(x, y)` and extent `(cx, cy)` become start `(x, y)` and end
`(x + cx, y + cy)` in points. True `flipH` swaps the x coordinates of the
endpoints; true `flipV` swaps their y coordinates. This applies reflection about
[the transform bounding box center](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.presentation.transform?view=openxml-3.0.1).
Both XML boolean spellings are accepted. Horizontal and vertical lines have a
zero extent on one axis; a zero-length line remains valid. Positions, extents
and widths all reuse `emu_to_points`; no new conversion helper is introduced.
Nonzero rotation remains unsupported. Rectangle flips retain 1C's omission
behavior; ellipse flips are also warned about and omitted.

Ellipses reuse solid/no-fill parsing and simple solid outlines. Lines reuse the
same outline color, width and supported style subset. Their absent shape fill
does not warn, because a line has no interior; explicit unsupported fill still
receives warnings. Scheme/theme colors, transformed colors, dashes, arrows,
compound/cap/alignment styles, adjusted/custom presets, inherited properties,
effects and other unsupported content continue to emit slide-scoped
`UnsupportedSlideContent` warnings. Unsupported paint is omitted without
substituting a color. Unsupported stroke preserves an independently supported
ellipse fill. Missing line stroke warns; zero width or explicit stroke `noFill`
emits no line, avoiding PDF hairlines. The existing 0.75-point omitted-width
default and its inheritance warning are retained. Malformed supported values
return errors before output is written.

## Generic primitives and rendering

`LineOp` already provides endpoints, color and positive width; no core line
change is needed. `EllipseOp { bounds, color }` and
`StrokeEllipseOp { bounds, color, width }`, with corresponding `DrawOp` variants,
are the only new core primitives. They express an axis-aligned filled ellipse
and its centered closed outline, without any PPTX types or preset names.

The PPTX adapter emits an ellipse fill followed by its outline, or one line
operation. It continues to emit backgrounds first and shapes in XML order. A
shape with no resolved paint retains its geometry but emits no operations.
Later shapes therefore cover earlier shapes, including mixed overlapping types.

The PDF backend constructs a closed ellipse from four cubic quarter-arcs,
using control distance `4 * (sqrt(2) - 1) / 3` times the corresponding radius
(approximately 0.55228475). Maximum radial approximation error is about 0.028%
of the radius. Coordinate conversion follows core's top-left convention; curve
intermediates use `f64` to avoid overflowing during otherwise valid arithmetic.
Validation checks finite bounding geometry and converted boundaries,
nonnegative extents and positive finite stroke width. Zero-size ellipses are
allowed, matching rectangle geometry policy. Every fill/stroke isolates its
graphics state; output remains deterministic. All path/PDF commands live solely
in `o2p-pdf`.

The fixture generator now also emits `one-ellipse`, `filled-ellipse`,
`outlined-ellipse`, `no-fill-ellipse`, `one-line`, `thick-line`, `mixed-shapes` and
`overlapping-shapes`. Tests exercise geometry, fill/stroke, width/color, signed
and nontrivial EMUs, flips, degenerate geometry, unsupported styles and malformed
input. Independent PDF decoding verifies cubic geometry, colors, widths,
coordinate conversion and overlapping paint order. The CLI mixed-shapes test
checks visible rectangle/ellipse/line commands and deterministic output. The
existing package, metadata, blank-page, rectangle, inspect and demo tests remain.
Text, images, groups, routed connectors, arrows, themes, masters, layouts,
gradients, transparency and the other deferred formats/features are not added.

# Milestone 1E: Embedded raster pictures

The pipeline remains PPTX XML -> normalized PPTX model -> format-independent
DisplayList -> PDF. The 1D baseline is `c97e0ce`. Package mechanics, metadata-only
parsing, inspection, backgrounds and vector geometry retain their behavior.

## Resource ownership and relationships

Core adds `ImageFormat::{Png, Jpeg}`, `ImageResource { format, bytes: Arc<[u8]> }`
and `ImageOp { rect, image: Arc<ImageResource> }` / `DrawOp::Image`. There are no
relationship IDs, package names or Office concepts in these types. Signature
detection identifies containers; validation and decoding belong to the backend.
Core remains dependency-free. Cloning operations or DisplayLists shares bytes.

Normalized `Shape::Image(ImageOp)` joins the existing ordered shape vector.
`Slide::shapes()` includes pictures; its historical name preserves the public
API. The adapter paints backgrounds first, then each vector shape or picture at
its XML position. Pictures can appear above or below existing shapes.

The bounded slide parser recognizes direct `p:pic`, `p:blipFill/a:blip` with
namespaced `r:embed`, and `p:spPr/a:xfrm/a:off,a:ext`. Transitional and Strict
namespaces and relationship types are accepted. It looks up IDs through
`OoxmlPackage::relationships(Some(slide_part))`, checks image relationship types,
and accesses canonical internal targets through `package.part()`. It never
inspects ZIP internals or reconstructs relative targets. External images are
warned about and never fetched. Missing/wrong references return `InvalidSlide`;
dangling internal targets remain errors from the existing package reader.

A parse-local cache keyed by media part name copies each supported referenced
media buffer once from the borrowed package API into an Arc. Repeated pictures,
slides, model clones and DisplayLists share this resource. The model outlives
the package without copying any entire package or retaining XML. Until the
package is released it holds the original part buffer alongside the one
normalized copy. Unreferenced media is neither copied nor decoded. No package
ownership rewrite, global registry or public resource-ID lifetime is needed.

## Placement and supported subset

Signed offsets and nonnegative dimensions reuse `emu_to_points` (f64 division by
12,700 followed by f32 rounding). The full image maps to the transform rectangle,
including differing aspect ratios. Absent stretch or empty `stretch/fillRect`
supports this mapping. Editing-only `picLocks/noChangeAspect` does not alter
explicit placement; DPI does not override the explicit dimensions.

Source rectangles, tiling, nonempty stretch destination rectangles, nonzero
rotation, true flips, placeholder pictures, nonrectangular/adjusted geometry and
image effects produce slide-scoped `UnsupportedSlideContent` warnings and omit
the picture. Other unsupported elements/attributes are warned about where
practical. Missing picture properties/transform/blipFill are warned about;
malformed references, duplicate supported metadata, invalid integers and
booleans fail. This is a supported subset, not complete OOXML schema validation.

PNG/JPEG signatures determine support independently of filename. Other formats
emit a structured warning naming the content type. A PNG/JPEG-declared part with
a bad signature fails parsing. Corrupt supported media fails decoding with
owned `PdfError::InvalidImage { page, op, message }` (zero-based indices). No
partial PDF is returned or written by the CLI. Animated PNG is explicitly
rejected by the backend rather than silently selecting a frame.

## PDF adapter, dependencies and limits

The private raster adapter uses `image` exactly 0.25.8, with defaults disabled
and only PNG/JPEG enabled. Its manifest declares Rust 1.85.0 and MIT OR Apache-2.0;
the pin preserves the workspace minimum Rust version. Decoder dependencies are
`png` 0.18.1 (MIT OR Apache-2.0) and `zune-jpeg` 0.4.21
(MIT OR Apache-2.0 OR Zlib). Color/math dependencies `moxcms`/`pxfm` permit
BSD-3-Clause OR Apache-2.0. Licenses were checked in downloaded manifests.
There are no GPL libraries or network/runtime services. PPTX uses `image` only
as a dev-dependency for original fixture generation. PDF reuses the existing
pure-Rust `flate2` configuration for raster stream compression. No third-party
types/errors cross library public APIs.

Destination geometry uses existing rectangle validation. Each shared resource
is decoded once and written as an RGB 8-bit Flate image XObject. JPEG follows
the same decode/RGB path, avoiding direct DCT/CMYK embedding complexity. PNG
alpha becomes a grayscale soft mask; opaque alpha needs no mask. Each draw saves
graphics state, applies `[width,0,0,height,x,page_height-y-height]`, paints the
XObject and restores state. Resources are deduplicated by Arc pointer identity
without hashing/copying media. Object reservation checks integer limits.

Before pixel decoding the adapter enforces hard limits of 16,384 per dimension,
16,777,216 pixels and 128 MiB decoder allocation/output. These complement OPC
part/compression limits and also protect generic core callers. They are decoder
bounds, not a total heap budget: pixel conversion, alpha splitting, compression
and PDF output require additional bounded buffers. Decoded buffers are released
after each resource is written. PDF bytes still accumulate in memory. Identical
input/backend versions remain deterministic.

## Validation

Six reproducible fixtures use original 2x2 RGBA color quadrants or a 3x2 green
JPEG. Tests independently inspect XObject dimensions, decoded pixels (with JPEG
tolerance), soft masks, matrices, resource sharing and overlapping paint order.
Parser tests cover Strict namespaces, canonical relative targets, missing/wrong/
external references, formats/crop/tile/effects, signed/fractional EMUs and bad
media. Backend tests bound dimensions/pixels before allocation. Existing shape,
metadata, package, inspect and demo tests remain regression coverage.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/image-with-shapes.pptx -o target/image-with-shapes.pdf
```

The CLI test checks raster/vector paint and deterministic output. The existing
`one-image.pptx` now renders its supported picture and retains the unsupported
hyperlink warning; its inspection behavior and fixture bytes remain unchanged.
Text, SVG, advanced cropping/effects, groups, themes, masters, layouts and other
document formats are not implemented.

# Milestone 1F: Basic text boxes

Baseline: `ddae310` (1E content baseline `c4d19cb`). The pipeline remains:

```text
PPTX XML -> normalized PPTX TextBox / Paragraph / TextRun / TextStyle
         -> basic layout -> generic core TextOp -> private PDF text adapter
```

No dependency changes, system fonts, external runtime processes or services are
introduced. OPC, picture decoding, vector geometry, metadata-only parsing,
inspection and blank-page APIs retain their behavior. The standard PDF fallback
is extended rather than replaced with a font discovery/substitution system.

## Owned model and paint order

`Shape::TextBox(TextBox)` is an additive variant. Existing rectangle, ellipse,
line and image variants and `TextOp` fields are unchanged. A text-bearing shape
becomes adjacent entries in `Slide::shapes()`: its existing vector geometry,
then its text box. This preserves fill -> outline -> text order within a shape
and XML order between shapes/images. A shape without a preset can contribute
only text if it has an explicit transform and no custom geometry. Unsupported
preset/custom geometry is still warned about and omitted as a whole. Explicitly
positioned text placeholders can render; their unresolved inheritance warns.
Missing transforms and unsupported shape rotations/flips retain omission behavior.

`TextBox { rect, insets, paragraphs }` preserves the existing shape transform's
x/y/width/height in points. `TextInsets` has left/right/top/bottom point values;
`Paragraph { runs, alignment, end_style }` preserves paragraph order, including
blank paragraphs. `TextRun { text, style }` owns literal Unicode and preserves
run order. A newline represents an explicit break. `TextStyle` owns font size in
points, bold/italic booleans and opaque sRGB color. `TextAlignment` has left,
center and right values. No raw XML, third-party or PDF types escape this model.

## Supported XML and fallbacks

The existing bounded slide XML adapter handles direct `p:sp/p:txBody`, `a:p`,
`a:r/a:t`, `a:rPr`, `a:br` and `a:endParaRPr` in both namespace families.
Literal whitespace, entities, CDATA and UTF-8/UTF-16 Unicode are preserved.
Missing/duplicate supported children, text nested inside `a:t`, stray text in
structural nodes, bad booleans, out-of-range sizes and malformed colors return
owned errors. XML controls other than newline (including literal tabs) return
an owned error: tab stops are not implemented. Existing byte/node/depth, DTD,
package and slide-count limits apply unchanged. Parsing releases each XML tree.

Direct `rPr/@sz` is hundredths of a point (100–400,000); `b` and `i` accept XML
boolean spellings. Direct `solidFill/srgbClr` requires six hex digits, without
transforms. Direct paragraph `pPr/defRPr` supplies local defaults, overridden by
the run. `endParaRPr` supplies style for blank paragraphs and trailing blank
lines, and never retroactively styles preceding runs. Direct `pPr/@algn` values
`l`, `ctr`, `r` are supported. Other alignment values warn and use left alignment.

Unresolved theme/master/layout properties use **18-point regular black sans-serif**,
left/top alignment and **7.2-point horizontal / 3.6-point vertical insets**.
Direct `bodyPr` lIns/rIns/tIns/bIns override insets (nonnegative signed-32-bit
EMUs). Each text box emits a slide-scoped `UnsupportedSlideContent` diagnostic
for unresolved inheritance/font selection, explicitly identifying the fallbacks.
Font typefaces and list-level styles remain unsupported and warn where present.
Unsupported colors use the paragraph/default fallback color with warnings.
A non-top vertical anchor warns and uses top placement; vertical text/nonzero
text rotation warns and omits text while preserving independently supported
vector geometry. Body wrapping/autofit, bullets, numbering, paragraph spacing,
underlines, strikes, superscripts, character spacing, kerning and other recognized
style attributes/children warn through the existing structured warning mechanism.
Unsupported fields are warned about and omitted; no field evaluation occurs.

## Minimal deterministic layout

Layout splits explicit newlines, accumulates runs horizontally, and places
paragraphs top-to-bottom within the inset rectangle. A line's baseline is its
top plus the largest run font size, with advance **1.2 × that size**. Empty
paragraphs/trailing blank lines use their end style. Left/center/right positioning
uses the combined line advance, not separately aligned runs. Unkerned fixed
sans-serif metrics live in dependency-free core and share the renderer's fallback
advances (missing glyphs measure as `?`). There is no wrapping, clipping to the
text box, shaping, bidi layout, font discovery or PowerPoint autofit.

Layout accumulates in f64, converting positioned operations to core f32 points.
Parsed sizes/geometry and existing XML limits bound layout arithmetic and work.
Text exceeding the inner width/height yields a structured `text box overflow`
warning during parsing. It still paints deterministically with ordinary PDF page
clipping; it never triggers autofit, truncation or a panic. Text layout is rebuilt
from the owned model when converting to DisplayList. The PDF renderer sees only
already-positioned generic runs.

## Generic text and Unicode PDF adapter

Core's existing `TextOp { origin, text, font, font_size, color }` remains source
compatible. `Font` adds logical bold, italic and bold-italic sans-serif choices.
PDF maps these to standard Helvetica, Helvetica-Bold, Helvetica-Oblique and
Helvetica-BoldOblique, using private font resources and WinAnsiEncoding. Printable
ASCII, Latin-1 including “Résumé”, and Windows-1252 Western punctuation/letters
are encoded explicitly; UTF-8 bytes are never passed to a Type1 text string.
Each run retains baseline coordinates, size and color, isolated graphics state,
paint order and deterministic output. Standard fonts are not embedded, so visual
font substitution across PDF viewers remains a fidelity limitation.

`o2p_pdf::render` preserves the strict unsupported-character error policy, now
accepting the larger WinAnsi repertoire. The additive `render_with_report` returns
`PdfReport { bytes, warnings }`; unsupported non-control Unicode scalars render
as `?`. `PdfWarning { page, op, missing_glyphs }` lists distinct scalars in first
occurrence order per operation (zero-based indices). Controls remain errors in
both APIs. For example **日本語 renders as ???**, with the literal Unicode retained
in PPTX/core models and a diagnostic naming the missing glyphs. Combining marks,
CJK, emoji, complex-script shaping and bidi text are not supported. This is a
bounded fallback policy, not full Unicode font coverage. `o2p convert` uses the
reporting API and prints both font and PPTX warnings before reporting success.

## Fixtures and validation

Eight original, deterministic synthetic fixtures are regenerated by the existing
PPTX generator: one-text-box, unicode-text, multiple-runs, multiple-paragraphs,
styled-text-basic, text-in-rectangle, text-with-image and overlapping-text-shape.
Tests cover literal extraction, paragraph/run order, size/bold/italic/color,
geometry/insets, local defaults/end styles, explicit breaks, alignment, overflow,
malformed markup, style warnings, namespaces/encoding/security limits, text-only
shapes and text mixed with all existing drawing types. Independent PDF decoding
checks font choices, WinAnsi bytes, extraction, positions/colors and paint order.
CLI tests verify readable text alongside vector and raster paint, deterministic
output, and Japanese fallback diagnostics. Earlier milestone tests remain.

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/text-with-image.pptx -o target/text-with-image.pdf
```

The test output is also rasterized with Poppler for visual QA during development;
Poppler is never a rendering dependency or a runtime engine subprocess. The
visible result contains a filled/outlined rectangle, the original quadrant image,
and readable “Hello Indonesia — Résumé” plus “Text and image”.


# Milestone 1G-A: Generic fonts, Unicode shaping, and embedding

Baseline: tagged `v0.1.0-alpha.1` (`7fc634d`). Completed package, vector, image,
inspection, and basic text paths remain intact. New configured pipeline:

```text
PPTX XML -> normalized PPTX text (including direct latin typeface)
         -> o2p-fonts request / configured resolver / Unicode shaping
         -> basic PPTX layout measured using shaped advances
         -> o2p-core GlyphTextOp / ShapedText / GlyphRun
         -> o2p-pdf embedded Type0 / CIDFontType2 -> PDF
```

`o2p-fonts` depends on core and privately adapts font/shaping dependencies. Core
remains dependency-free. PPTX passes generic family/weight/style requests and
positions shaped output; PDF validates and serializes it. No PPTX code emits PDF
commands, and PDF does not resolve fonts or shape text. No global platform font
registry is required. File, byte-buffer, and configured-directory registration
serve desktop, bundled mobile, and future platform-provider consumers alike.

Core owns shared full font bytes with `Arc<FontResource>` and UTF-8 source ranges
for positioned glyphs. Registry models and DisplayLists share bytes and have no
package/parser lifetime dependency. Resolution chooses requested faces, explicit
compatible aliases, then configured fallback families, with stable style and
registration-order ties. Substitution and missing-font/glyph warnings are owned,
structured results. Grapheme boundaries prevent splitting combining sequences.

The shaping boundary is deliberately before DisplayList construction, so layout
and PDF cannot disagree about selected-font advances. `GlyphTextOp` retains
logical Unicode independently of visual glyph order. Advances/offsets, segment
baselines and font sizes use points; geometry/paint order remain unchanged.
Existing `TextOp` and no-configuration CLI calls retain the Helvetica/WinAnsi
behavior, including strict errors and reporting replacement. Adding owned family
information makes PPTX `TextStyle` Clone rather than Copy.

PDF embeds each font resource once, in full, without subsetting. Type0/Identity-H
fonts reference CIDFontType2 descendants, descriptors, widths, explicit
CIDToGIDMap and UTF-16BE ToUnicode mappings. CIDs distinguish glyph/source pairs.
ActualText preserves the semantic run for combining clusters and logical bidi
text; glyphs remain selectable PDF text. Reader-dependent RTL extraction and
lopdf's lack of ActualText support are documented, not hidden.

HarfRust 0.14.0 (MIT) is the focused maintained pure-Rust shaper; ttf-parser 0.25.1
(MIT OR Apache-2.0) supplies bounded parsing/metrics/rights checks. Unicode bidi,
script and grapheme helpers support deterministic segmentation. All versions
fit Rust 1.85. No system discovery, fontconfig, rasterizer, native service, cloud
or process dependency is introduced. Static standalone TrueType/glyf is the
supported embedding path; CFF/CFF2, variable, color, bitmap and collection fonts
return structured errors. Font and text workloads have explicit ceilings.

See [FONTS.md](FONTS.md) for ownership, matching policy, dependency/license
comparison, security limits, Unicode/extraction strategy and precise limitations.
See [fixture licenses](../fixtures/fonts/README.md) for the small OFL test fonts,
including a 3 KiB Japanese case. Test-asset preparation is independent of runtime
font subsetting (still deferred). Unicode fixtures include ASCII, Résumé,
Greek/Cyrillic, ligatures/combining marks, Arabic and 日本語. Tests retain all prior
regressions and verify metadata, fallback/styles/warnings, glyph order, object
embedding, source mapping, independent extraction and mixed paint order.

Themes, master/layout font inheritance, wrapping/reflow/autofit, character
spacing, bullets/numbering, rotation/vertical text, paragraph-wide cross-style
bidi, font subsetting and unsupported font technologies remain deferred. DOCX,
XLSX, Flutter, charts and SmartArt are unchanged and unimplemented.


# Milestone 1G-B: Measured text wrapping and paragraph layout

Baseline: committed `359f092` (Milestone 1G-A). The configured-font pipeline is:

```text
PPTX normalized text semantics / textbox geometry
  -> generic o2p-layout paragraphs / font requests / inset rectangle
  -> o2p-fonts Unicode shaping / cluster ranges / glyph advances
  -> o2p-layout measured lines / positioned styled glyph runs
  -> o2p-core DisplayList -> unchanged o2p-pdf serialization
```

The new small `o2p-layout` crate depends on core, fonts and the already-present
permissively licensed `unicode-segmentation`. Core remains dependency-free;
fonts are unchanged; PDF still only validates/serializes drawing operations.
A separate crate avoids adding a shaping dependency cycle to core and avoids
embedding format-specific layout inside PDF. This is an incremental extraction
of configured text placement, not a font subsystem rewrite.

The generic contract uses owned styled `TextRun`, `Paragraph` and measured
`Line` values. `layout()` receives an already-inset rectangle and a resolver,
returning positioned core glyph runs, font diagnostics and overflow state.
PPTX translates its existing family/weight/italic/color/size and alignment into
these types; public PPTX/core/PDF APIs remain available without changed fields.
Its adapter emits lines in place of the original textbox, preserving
fill/outline/text and shape/image z-order.

Mandatory breaks split first. Runs are shaped once for glyph measurements;
logical UTF-8 intervals are intersected with full-text grapheme boundaries to
form safe wrapping atoms. Greedy wrapping prefers fitting whitespace and uses
cluster boundaries for long tokens. Whitespace is retained and measured.
Chosen fragments are reshaped at actual line edges and rechecked, retreating
when context changed their width. Single oversized clusters still advance the
cursor and produce overflow. No scalar-count/ASCII-width estimate is involved.
Styles and logical source mappings survive fragmentation, including ligatures,
combining marks and fallback-font segments.

Each final line's width is the sum of emitted shaped advances. Left/center/right
x offsets are respectively 0, half the remaining inner width, or all of it.
The baseline is top + maximum nonempty run size; line advance is 1.2 times
that size. Blank lines use the resolved paragraph end size. All paragraph lines
stack consecutively from the top inset; no PowerPoint spacing/autofit is implied.
Vertical sizing is nominal, not an ink-bound calculation.

One deterministic structured `TextLayoutWarningKind::Overflow` is returned per
textbox whose final line width or nominal stacked line height exceeds its
inset rectangle. Text remains present without textbox clipping or shrinking;
PDF's existing page-boundary clipping is unchanged. Parser overflow diagnostics
are explicitly labeled legacy/unwrapped to distinguish the compatibility path.
Square/default wrapping works with configured fonts. Explicit no-wrap/unknown
modes warn and use wrapping; the unchanged legacy API remains unwrapped.

Unicode scope is whitespace-based, with grapheme/shaping-cluster-safe emergency
breaks and line-edge reshaping. Full Unicode breaking, hyphenation, cross-style
shaping and paragraph-wide bidi remain deferred. Explicit byte/run/paragraph
and reshaping-work limits return owned errors; invalid geometry never produces
nonfinite positioned output. See [FONTS.md](FONTS.md) for the algorithm, limits,
line-height/alignment formulas, whitespace policy and precise Unicode caveats.

Nine reproducible PPTX wrapping fixtures, generic layout tests, integrated
PPTX/PDF tests and a CLI mixed-style test cover measurement, exact fits,
whitespace/explicit breaks, oversized clusters/words, paragraphs/empty lines,
all alignments, styles, source safety and deterministic overflow. Existing
text/font/shape/image/z-order tests remain. Embedded fonts, ToUnicode and
ActualText are verified independently; visual Poppler QA checks mixed styling,
center/right lines and accented Latin/Japanese/Arabic. No advanced typography,
themes/masters/layouts, DOCX/XLSX, Flutter, charts or SmartArt is introduced.


# Milestone 1H: Individual element transforms

Baseline: committed `d79950b` (Milestone 1G-B). The pipeline is:

```text
PPTX a:xfrm -> normalized Shape::Transformed (geometry + optional text)
             -> generic Transform / DisplayList scope -> PDF graphics matrix
```

No dependencies, font/layout changes, package changes, or architectural rewrite
are introduced. `o2p-core` remains dependency-free and contains no PPTX fields.
All original position and size fields remain in page coordinates. Identity
transforms keep the original normalized variants and drawing operations.

## Coordinate system and composition

Core's `Transform { a, b, c, d, e, f }` is a six-coefficient affine matrix:
`x' = a*x + c*y + e`, `y' = b*x + d*y + f`. Coordinates are points, origin is
at the top left, x grows right and y grows down. Positive rotation is clockwise.
Generic helpers provide translation and rotation/reflection about a center.
The PPTX adapter reads explicit `a:xfrm/off` and `ext` with the existing bounded
EMU conversion; extents describe the element BEFORE rotation, never a rotated
axis-aligned bounding box. Zero extents and signed offsets remain valid.

DrawingML `rot` is a signed ST_Angle integer in **1/60,000 degree** units.
`drawingml_rotation_to_degrees` centralizes conversion: normalize integer units
modulo 21,600,000, then divide in f64 and round to f32. Thus -90 degrees is
270 degrees and full turns are identity. Exact quarter turns use exact sine and
cosine values. Malformed/out-of-range signed i32 angles and invalid booleans
return owned `InvalidSlide` errors. Missing rotation/flips use zero/false;
XML boolean spellings 0/1/false/true are accepted.

For offset `o=(x,y)` and local center `c=(width/2,height/2)`, using column vectors:

```text
M_local = T(o) * T(c) * R(clockwise degrees) * F(flipH,flipV) * T(-c)
F = diag(flipH ? -1 : 1, flipV ? -1 : 1)
```

This is the graphics composition order: translation -> center adjustment ->
rotation -> flip -> centered local drawing. Matrices act right to left on points:
subtract the local center, flip along local axes, rotate, restore the center,
then translate to the slide. Flips occur BEFORE rotation; reversing R and F
changes a single-flip + 90-degree result. Both flips equal a 180-degree rotation
about the same center. This follows the individual-shape transformation subset
in the [ECMA-376 primer](https://c-rex.net/samples/ooxml/e1/Part3/OOXML_P3_Primer_Shape_topic_ID0EDMMO.html).

Existing primitives and laid-out text already contain their slide offset. The
adapter therefore emits the equivalent page-coordinate matrix:
`M_page = T(o+c) * R * F * T(-(o+c))`. The intended center `(x+width/2,y+height/2)`
is fixed; no element rotates around the page origin. Text insets, baselines and
wrapping use the original bounds and are transformed only after layout.

## DisplayList and PDF

`DrawOp::BeginTransform(Transform)` and `EndTransform` delimit one individual
element's fill, outline, image and/or text in their existing order. Normalized
`Shape::Transformed { transform, element }` owns only that element's supported
geometry and text, not a DrawingML group. Straight lines use the same transform
path as rectangles, ellipses and pictures, replacing the old endpoint-only flip
implementation with equivalent visual geometry. No XML element is reordered.

Scopes are flat, balanced within each page and cannot nest in this milestone.
The renderer rejects nested/unbalanced scopes, singular/nonfinite matrices,
PDF matrix conversion overflow and nonfinite transformed drawing positions with
indexed `PdfError::InvalidGeometry`. Resource collection still visits the flat
paint sequence; images and fonts remain shared and serialized once.

All primitive renderers retain their existing bottom-left PDF placement. With
`P(x,y)=(x,page_height-y)`, PDF uses `P * M_page * P`:
`[a,-b,-c,d,c*page_height+e,page_height*(1-d)-f]`. Each element emits `q`, `cm`,
its original drawing operations (each with its original local q/Q), and `Q`.
The next element cannot inherit its matrix. The backend sees only generic
transforms and does not know the input format.

Images stay native PDF image XObjects; no slide rasterization occurs. Rotated
and reflected text remains real text: the existing standard-font or Type0 /
CIDFontType2 path, FontFile2, CIDToGIDMap, ToUnicode and ActualText are unchanged.
Rotation/reflection applies to the complete text-bearing element, including
its text. Reader-specific mirrored/rotated selection and RTL extraction remain
viewer-dependent; semantic source mappings are retained. See FONTS.md.

## Supported subset and validation

Nine minimal synthetic fixtures cover 45/90-degree rectangles, ellipse, image,
wrapped Unicode text, horizontal and vertical flips, both flips with rotation,
and a mixed slide with vector, raster and text paint. The asymmetric 2x2 color image
and ABC labels make orientation visible. Tests verify parsing/unit conversion,
centers, noncommuting composition, all primitives, fill/outline/text scopes,
z-order, image resource reuse, unchanged wrapping and Unicode extraction,
malformed matrices/values and all completed-milestone regressions.

```sh
cargo run --offline -p o2p-cli -- convert fixtures/pptx/mixed-transforms.pptx -o target/mixed-transforms.pdf
cargo run --offline -p o2p-cli -- convert fixtures/pptx/mixed-transforms.pptx -o target/mixed-transforms-fonts.pdf --font-dir fixtures/fonts
```

Groups, nested coordinate spaces and root group transforms remain unsupported;
only the existing empty/all-zero root group scaffolding is accepted. Unknown
transform attributes/children (including chOff/chExt on individual elements)
produce structured slide warnings and omit the element. Shape 3D scene/properties
warn and omit the element; picture effects retain existing omission behavior.
Independent bodyPr text rotation, vertical writing, skew, perspective, 3D,
advanced connectors, inheritance/themes/masters/layouts, charts, SmartArt and
animations remain unsupported. These features produce structured warnings;
malformed supported values produce errors. DOCX, XLSX and Flutter remain out of
scope. Full Office typography and inheritance fidelity are not implied.

Development QA: `cargo fmt`, workspace clippy with all targets/features, and all
197 workspace tests passed. Poppler-rendered default/configured mixed slides and
the rotated Unicode textbox were visually inspected: clockwise centers,
quadrant-image orientation, reflected ABC text, shared borders/text and wrapped
Unicode fit as intended. Independent Poppler extraction recovered rotated
Résumé/日本語/ffi and the mirrored ABC labels. Poppler is a development QA tool,
not a runtime dependency.

# Milestone 1I-A: Presentation themes and generic resolution

Baseline: `f392e61`. The additive semantic pipeline is:

```text
OPC officeDocument -> presentation theme relationship -> bounded DrawingML XML
  -> owned PPTX Theme / ColorScheme / FontScheme
  -> shared color resolver / explicit Latin theme font resolver
  -> existing PPTX Slide / TextStyle -> generic fonts/layout -> DisplayList -> PDF
```

`o2p-ooxml` retains all relationship/path/package mechanics. `o2p-pptx` loads the
presentation's explicit internal theme relationship through `relationships()`
and `part()`, without ZIP inspection or conventional-path assumptions. The theme
uses the same XML security limits and adapter as slide content. Only
content-aware parsing loads it; metadata-only parsing/inspection stay opaque.
The optional owned theme is exposed by the additive `Presentation::theme()` API.
Malformed supported theme values add `PptxError::InvalidTheme`; unsupported theme
semantics add `PptxWarning::UnsupportedTheme`. Existing APIs keep their contracts.

All twelve standard color slots support sRGB or deterministic system `lastClr`.
One adapter handles direct/scheme color selection and tint, shade, lumMod/lumOff
for backgrounds, fills, outlines, lines and text. Unavailable paint preserves
the prior omission/local-text-color fallback with structured warnings. Direct
sRGB never depends on theme availability. Master color-map aliases are deferred.

Major/minor Latin and available script mappings are normalized. Explicit
`+mj-lt`/`+mn-lt` tokens become ordinary requested family names, so absent fonts
retain generic substitution diagnostics. No PPTX token semantics enter fonts,
layout, core or PDF. Absent run typefaces do not synthesize theme inheritance.
The only slide/text integration changes replace the existing shared color and
explicit typeface parsing points; drawing/layout/serialization remain intact.

No dependency changes or architectural rewrite. Presentation-scoped theme
discovery does not follow masters/layouts, select per-master themes, inherit
placeholder properties, or interpret the style matrix. See [THEMES.md](THEMES.md)
for supported slots/transforms, priority, errors, fallbacks, discovery limits and
deferred semantics. Ten synthetic fixtures and parser/math/font/PDF/CLI tests
extend completed-milestone regressions.

# Milestone 1I-B: Slide, layout and master inheritance

Baseline: committed `578453b`. The new fixed-depth semantic stage follows
slide -> layout -> master -> theme through existing OPC relationships. Owned
`SlideLayout`, `SlideMaster` and `Placeholder` values retain sparse geometry/text
properties and separate ancestor decorations from placeholder content. Layouts
merge master defaults, then slides merge layouts, with local properties always
winning. Only effective normalized `Slide`/`Shape`/`TextStyle` values proceed to
generic layout, DisplayList and the unchanged PDF backend.

Shared sources and image resources are cached per presentation; bounded XML
parsing applies independently to each reachable part. Missing links warn; invalid
discovered links fail. Effective background precedes master decorations, layout
decorations and slide content, preserving XML order within each layer. Ancestor
placeholder prompts never render. Common shape visibility controls are supported.
Per-master themes use the existing centralized resolver; no dependency or generic
rendering changes are introduced.

See [INHERITANCE.md](INHERITANCE.md) for exact matching, sparse-property precedence,
source model APIs, theme selection, visibility, background rules and limitations.
The full txStyles hierarchy and DrawingML style matrix remain deferred to later
milestones. Required missing-layout warnings are now present for earlier decks
without layouts; their deterministic rendering contracts remain unchanged.

# Milestone 1I-C: DrawingML format styles and fidelity cleanup

Baseline: committed `336f4e2`. The additive PPTX path is style references ->
scoped theme fmtScheme -> effective fill/line/background and font defaults ->
existing normalized Slide/Shape/TextStyle -> generic layout/DisplayList -> PDF.

`Theme::styles` owns a minimal StyleMatrix with ordered fill, line and background
collections, including unsupported slots. Deferred owned colors reuse the
central theme resolver/transform math with reference phClr substitution. Raw XML
remains private. Split fill/background and one-based line indexing belong only
to `o2p-pptx`. Reference defaults stay separate from sparse direct properties
through inheritance so direct local and inherited formatting wins per property.
The existing sparse line adapter is shared with style-bearing ordinary shapes.

OPC mechanics, generic fonts/layout, DisplayList and PDF remain unchanged; no
dependencies are added. Rectangles, ellipses, simple lines and text-bearing
shapes gain solid/no-fill theme styles. Background presence/visual layering
continues to follow 1I-B. Unsupported styles warn and omit affected paint rather
than being approximated. Effect styles, gradients, patterns, decorations and
PowerPoint text hierarchy remain deferred. See [STYLES.md](STYLES.md) for the
model, exact indexing, precedence, warnings, fallbacks and validation fixtures.
