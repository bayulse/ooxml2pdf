# Fonts and text fidelity — Milestones 1G-A / 1G-B / 1H / 1I-A

Baseline: `v0.1.0-alpha.1` / `7fc634d`. This milestone adds configured Unicode
text without replacing the completed package, shape, image, or legacy text paths.
It does not implement full PowerPoint typography.

## Registration and resolution

`o2p-fonts::FontBook` registers standalone fonts from `Arc<[u8]>`, explicit file
paths, or configured directories. Directories are nonrecursive, sorted by path,
and skip symlinks. All failed registrations return owned `FontError` values;
directory registration retains successful earlier entries if a later font fails.
There is no system-font discovery, global registry, remote fetch, or automatic
bundled font. Mobile consumers can supply bundled/platform bytes or implement
the `FontResolver` trait with validated `FontFace` instances.

`FontRequest` contains family, OpenType weight (1–1000), and italic style.
Matching uses trimmed, case-insensitive family names. Selection order is:

1. Requested family, ranked by matching italic style, nearest weight, then
   registration order. Prefer a face covering the complete grapheme cluster.
2. Explicit configured compatible family aliases, in configured order.
3. Explicit configured fallback families, in configured order and style rank.
4. If none covers the cluster, use the first eligible face's `.notdef` glyph
   and emit `MissingGlyph`. If no eligible face exists, return `NoFonts`.

`MissingFont` reports absent requested families. `Substitution` records the
requested family/weight/style and selected family/weight/style, including generic
sans-serif resolution, aliases, style approximation, and coverage fallback.
Warnings are deduplicated per semantic run/text box in first-occurrence order.
Fallback is at extended grapheme boundaries, preserving combining sequences.
There is no synthetic bold/italic. No Office-specific alias table is built in.

```rust
let mut fonts = o2p_fonts::FontBook::new();
fonts.register_bytes(bundled_font_bytes)?; // Arc<[u8]>
fonts.set_fallback_families(vec!["My Sans".into(), "My CJK".into()]);
let layout = presentation.to_display_list_with_fonts(&fonts)?;
let pdf = o2p_pdf::render_with_report(&layout.display_list)?;
// Retain presentation.warnings(), layout.warnings, and pdf.warnings.
```

Consumers must have permission to embed supplied fonts. The adapter rejects
restricted/invalid OS/2 embedding permissions and bitmap-only embedding rights;
full embedding accommodates fonts forbidding subsetting.

## Shaping and ownership

`o2p-fonts` privately adapts HarfRust and ttf-parser. No third-party types appear
in public signatures. HarfRust prepares cached immutable font data. The registry,
normalized output, and cloned DisplayLists share font bytes with `Arc`; output
outlives both the font registry and the OOXML package.

Semantic PPTX runs remain in `o2p-pptx`. Direct `a:latin/@typeface` is retained in
`TextStyle::family` (which now owns a string and is `Clone`, not `Copy`). Local
paragraph defaults are retained. Explicit `+mj-lt` / `+mn-lt` references resolve
to major/minor Latin families from the presentation theme in `o2p-pptx`, before
creating a generic requested family. Unsupported aliases or unavailable theme
families warn and use configured sans-serif. `a:ea` / `a:cs` typeface selection,
full master text-style hierarchies, and embedded OOXML fonts remain unsupported and
warn. No PPTX XML concepts enter `o2p-fonts` or `o2p-core`.

Theme resolution requests the actual family even if it is unregistered;
the generic resolver retains its existing `MissingFont` and `Substitution`
diagnostics and deterministic configured fallback order. Major/minor script
mappings are preserved in the theme model but do not infer run fonts. Only
explicit Latin tokens resolve, including supported inherited placeholder defaults.
Absent typefaces use those defaults before the generic request. See
[INHERITANCE.md](INHERITANCE.md) for Milestone 1I-B's exact precedence and limits.
The standard-font compatibility path retains Helvetica selection; configured
fonts are required to render the resolved family. See [THEMES.md](THEMES.md) for
discovery, aliases, color semantics and unsupported inheritance.

```sh
cargo run --offline -p o2p-cli -- convert fixtures/pptx/theme-colors-and-fonts.pptx --font-dir fixtures/fonts -o target/theme-colors-and-fonts.pdf
```

Shaping processes one semantic run at a time, segments Unicode bidi directions
and scripts, chooses fonts at grapheme boundaries, and returns visual glyph
order. Glyphs preserve IDs, advances, offsets, and UTF-8 source ranges. One glyph
can cover several code points, and several glyphs can share a source cluster.
All positions are scaled to points with downward-positive y before entering core.

`DrawOp::GlyphText` owns `ShapedText` with original logical source, font size,
total advance, and ordered `GlyphRun` resources/relative baselines. This is the
layout/PDF boundary: shaping and resolution precede DisplayList construction;
PDF performs no resolution, shaping, or Office layout. Existing `TextOp` remains
the standard-font compatibility primitive.

Configured PPTX text now passes through `o2p-layout` for measured wrapping and
paragraph placement before becoming core glyph operations. The font subsystem
and PDF serialization remain unchanged. The legacy `to_display_list()` API and
CLI without explicit font configuration retain the historical standard-font,
unwrapped behavior. Use `to_display_list_with_fonts()` or CLI `--font` /
`--font-dir` to obtain Unicode shaping and wrapping.

## Measured line layout (Milestone 1G-B)

`o2p-layout` accepts generic `TextRun` (literal text, `FontRequest`, font size,
color), `Paragraph` (runs, alignment, empty-line size) and an already-inset point
rectangle. Its report contains measured `Line` values, positioned `GlyphTextOp`
runs, deduplicated font warnings and an overflow flag. PPTX owns the conversion
from direct paragraph/run properties and textbox insets; no Office type enters
layout, fonts, core or PDF. There is no new third-party dependency: layout reuses
`unicode-segmentation` 1.12.0 (MIT OR Apache-2.0), already present in the lockfile.
It adds no Unicode line-breaking library or mobile/runtime service.

The algorithm is deliberately limited:

1. Split mandatory newlines, keeping blank and trailing lines. Shape each
   remaining styled fragment with the existing `o2p-fonts::shape` pipeline.
2. Sort glyph source intervals into logical UTF-8 order, independently of visual
   glyph order. Intersect safe shaping boundaries with extended grapheme
   boundaries of the concatenated text, including across style boundaries.
3. Accumulate actual glyph advances to find a fitting prefix. Prefer its last
   whitespace boundary when more text follows; otherwise break at the last
   fitting cluster. Nonbreaking U+00A0/U+202F spaces are not preferred breaks.
   Preserve all whitespace, including separators at line ends; they participate
   in measurement/alignment and remain in ActualText. Style boundaries alone
   are not word boundaries. Exact fits are allowed (the estimate tolerates only
   f32 rounding; final emitted widths are checked without that tolerance).
4. Reshape the chosen fragments with their original font requests, sizes and
   colors, using the actual line-edge context. If their final total advance
   exceeds the box, retreat to an earlier whitespace/cluster boundary and
   remeasure. A single oversized cluster is emitted with overflow; every
   iteration consumes at least one cluster. No character-count or ASCII-width
   estimate is used. Each final run owns new source-relative cluster ranges,
   keeping ToUnicode and ActualText valid after wrapping.

For every laid-out line, width is the f64 sum of its emitted
`ShapedText.advance.x` values. Starting from the inner left edge, alignment adds
0 for left, `(inner_width - line_width) / 2` for center, or
`inner_width - line_width` for right. Adjacent styled fragments advance by their
own shaped widths and share a baseline. Alignment never uses pre-wrap paragraph
width. Font resources, weight/italic choice, color and size survive each split.

The line-height policy intentionally preserves the preceding milestone:
baseline = line top + largest nonempty run font size; line advance =
**1.2 times that size**. Blank paragraphs and blank explicit-break lines use
`endParaRPr`'s resolved size. Paragraphs stack their lines consecutively without
additional before/after spacing. This is nominal line-box layout, not a promise
that every font's ink bounds fit inside that line box; unusual ascenders,
descenders and mark extents are not used for vertical spacing in this milestone.
Horizontal advances come entirely from font shaping/metrics.

Width and nominal line-box bottom are checked against the inset rectangle.
PPTX reports one `TextLayoutWarningKind::Overflow` per affected textbox,
identified by zero-based page and textbox indices, in deterministic order.
All laid-out lines remain present, even below the box. There is no textbox
clipping, truncation, autofit or font shrinking. Ordinary documented PDF page
clipping still applies at the page edge. Historical parser diagnostics now say
**legacy text box overflow**: they concern the unwrapped compatibility API and
are separate from configured-font warnings measured after wrapping.

Direct/default `bodyPr` square wrapping is supported on the configured path.
Explicit `wrap="none"` receives a structured unsupported-behavior warning and
uses wrapping there; the model has no new field that would break existing
`TextBox` struct construction. The legacy path continues explicit breaks only.
Unknown wrapping modes, autofit, non-top anchors, independent bodyPr rotation/vertical text,
bullets/numbering, spacing, decorations and other recognized unsupported
properties retain structured parser warnings. Independent bodyPr rotation and vertical writing retain
their documented omission behavior. Element a:xfrm rotation is supported by 1H.

Unicode limitations: this is whitespace wrapping with cluster-safe emergency
breaks, not UAX #14, CJK punctuation rules, hyphenation or language-specific word
segmentation. Emergency breaks may split a word/nonbreaking-space sequence,
but cannot split the original shaping cluster or an extended grapheme. Shaping
and bidi remain per styled fragment; paragraph-wide bidi ordering and shaping
across style changes remain deferred. Reshaping at line edges preserves Arabic
joining within fragments; reader-dependent RTL extraction remains as described
below. Font coverage still depends on the explicitly configured fonts.

Layout rejects invalid/nonfinite geometry and sizes with owned `FontError`
values. Work is bounded to 16,000 bytes/runs per paragraph, 16,000 paragraphs,
65,536 runs and 1 MiB source text per textbox. Additional partial-fragment
reshaping is capped at 1 MiB processed bytes per paragraph, preventing
pathological retreat from repeatedly shaping unbounded prefixes. These limits
complement existing package, XML, shaping and PDF ceilings; they are not an
exact total heap budget. Exceeding a limit returns a structured error rather
than truncating text.

```sh
cargo run --offline -p o2p-cli -- convert fixtures/pptx/mixed-runs-wrapped.pptx -o target/mixed-runs-wrapped.pdf --font-dir fixtures/fonts --fallback-family "O2P Test Sans"
```

The nine wrapping fixtures cover short/long paragraphs, mixed runs, blank
paragraphs, center/right alignment, oversized words, vertical overflow and
accented Latin/Japanese/Arabic with the existing licensed fonts. Generic layout,
PPTX/PDF and CLI tests verify width, source/cluster safety, styling, positioning,
deterministic diagnostics, embedding and extraction. Development QA renders
mixed, center/right and Unicode PDFs with Poppler, checks their visible layout,
and independently extracts searchable text. Poppler remains a development tool.

## PDF representation and extraction

`o2p-pdf` validates font resources and glyph/source/position data, then embeds
each shared font resource once as its complete, uncompressed TrueType program
in `FontFile2`. There is no runtime subsetting or outline/raster conversion.
Fonts use Type0 + Identity-H + CIDFontType2, font descriptors/widths, and explicit
CIDToGIDMap streams. Stable paint-order resource IDs avoid pointer-order output.

CIDs are keyed by **glyph ID and source sequence**, not glyph ID alone. Thus
the same glyph used for decomposed and precomposed Unicode has distinct mappings.
A type 2 ToUnicode CMap maps CIDs to UTF-16BE sequences (including surrogate
pairs), in blocks of at most 100 mappings. For one-to-many clusters, the first
glyph maps the whole source; continuation glyphs have no separate Unicode
mapping. Each semantic operation also has UTF-16BE `ActualText` containing its
original logical source, recovering complex clusters and logical bidi text in
readers supporting marked-content replacement. Positioned glyphs remain real
PDF text with text matrices incorporating shaping offsets and advances.

Tests independently inspect objects and mapping streams and extract Latin,
Greek/Cyrillic, ligatures, and Japanese with lopdf. Optional Poppler tests check
copy/search recovery of combining clusters and Japanese. `lopdf` 0.38 ignores
ActualText and substitutes U+FFFD for unmapped continuation glyphs; it therefore
cannot validate complex-cluster copy behavior alone. Poppler raw RTL extraction
uses directional controls/visual ordering, so exact logical RTL extraction is
validated through ActualText contents rather than claiming uniform behavior
across viewers. Reader-specific RTL copy/search fidelity remains a limitation.

Missing embedded glyphs render `.notdef` with `EmbeddedMissingGlyph` warnings;
the original Unicode stays in ToUnicode/ActualText. Strict `render` rejects them.
Legacy `TextOp` keeps Helvetica/WinAnsi, strict errors, and reporting `?` warnings.
With no font flags/configuration, existing CLI behavior is preserved. To use
embedding: `o2p convert input.pptx -o output.pdf --font <file>` or `--font-dir <dir>`.
Flags can repeat; `--fallback-family` specifies family order. Without that flag,
the CLI uses distinct registered families in registration order (all explicit
files first, then sorted directories). No platform-dependent defaults are used.

## Dependencies, format limits, and security

Chosen dependencies:

| Dependency | Role | License / rationale |
| --- | --- | --- |
| HarfRust 0.14.0, std only | Unicode/OpenType shaping | MIT, maintained pure-Rust HarfBuzz port; Rust 1.85 |
| ttf-parser 0.25.1, std only | Metadata, cmap, metrics, format/rights validation | MIT OR Apache-2.0; zero-allocation parsing |
| unicode-bidi 0.3.18 | Run direction ordering | MIT OR Apache-2.0 |
| unicode-script 0.5.8 | Script segmentation | MIT OR Apache-2.0 |
| unicode-segmentation 1.12.0 | Grapheme-preserving fallback | MIT OR Apache-2.0; pinned for workspace MSRV |

ttf-parser alone cannot shape. cosmic-text is maintained and permissively
licensed, but its font database, layout, optional rasterizer/fontconfig and
locale integration exceed this milestone. rustybuzz is smaller/focused but
archived upstream in favor of HarfRust. HarfRust uses read-fonts/font-types
(MIT OR Apache-2.0); their sharing helpers include yoke/zerofrom (Unicode-3.0),
also compatible permissive licenses. Other new support crates are permissively
licensed; no GPL dependency, native service, fontconfig, rasterizer, ICU feature,
JavaScript, network runtime, or OS font backend is added. Cargo.lock pins the
resolved graph. This is a qualitative size choice, not a measured APK/IPA size
claim. Runtime remains platform-neutral Rust; Android/iOS cross-build validation
is deferred. Fixtures are approximately 361 KiB and never linked into binaries.

Supported: standalone static sfnt TrueType, including OpenType containers with
`glyf` outlines. CFF/CFF2, variable `fvar`, color COLR/CPAL/SVG, embedded bitmap
CBDT/CBLC/EBDT/EBLC/bdat/bloc/sbix, collections, WOFF/WOFF2 and invalid fonts are
rejected with structured errors. Extensions absent from the supported path are
not silently flattened into outlines or a default face.

Hard limits: 16 MiB per font, 64 MiB registered/embedded font bytes, 256 fonts,
16,000 UTF-8 bytes per semantic shaping operation, 262,144 output glyphs per
operation, 256 UTF-16 units per PDF source cluster, and 65,535 CIDs per PDF font.
Text/cluster limits keep PDF text-string and ToUnicode string lengths valid.
Registration bounds file reads before allocation and rechecks byte-buffer input.
Font table parsing is bounds-checked; shaping errors propagate as owned errors.
HarfRust has internal operation/output-growth ceilings. These are workload
bounds, not a strict total heap budget; shaping caches and PDF buffers add memory.
Mutation/truncation tests are smoke coverage, not a sustained fuzzing campaign.

See [fixture licensing and regeneration](../fixtures/fonts/README.md).


## Milestone 1H — Rotation and reflection fidelity

Individual `a:xfrm` transforms now rotate/reflect the entire text-bearing shape,
including fill, outline and every wrapped text run, about the shape center.
Flips use local axes before clockwise rotation. Layout still measures the
original inset rectangle; rotation never rewraps to the rotated axis-aligned
bounds. The DisplayList surrounds the original text operations with one generic
graphics scope, and the PDF backend applies a native matrix.

Font bytes/resources, glyph IDs, advances, shaping source ranges, Type0 fonts,
ToUnicode and ActualText remain unchanged. Text remains selectable/searchable,
including rotated Japanese, accented Latin, ligatures and combining sequences
when configured fonts cover them. The unconfigured Helvetica path keeps its
existing Unicode coverage/replacement warnings. Images remain image XObjects.
Mirrored text is intentionally mirrored with its containing geometry in this
supported subset; logical Unicode mappings remain in source order. Readers may
handle mirrored selection rectangles, rotated reading order or RTL copy/search
differently. No claim of identical copy order across all viewers is made.

`rotated-text.pptx` exercises wrapped Résumé/日本語/ffi. Independent PDF tests
verify embedded FontFile2, ToUnicode, ActualText and text extraction. Native
rotation/reflection affects graphics state, never font resolution or layout.
Independent `bodyPr@rot`, vertical writing and WordArt remain unsupported with
structured warnings; they are distinct from the supported element transform.
