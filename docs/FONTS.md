# Fonts and text fidelity — Milestone 1G-A

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
paragraph defaults are retained. `+mj…` / `+mn…` theme references warn and use the
configured generic sans-serif request. `a:ea` / `a:cs` typeface selection,
theme/master/layout inheritance, and embedded OOXML fonts remain unsupported and
warn. No PPTX XML concepts enter `o2p-fonts` or `o2p-core`.

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

PPTX configured layout preserves explicit paragraphs/breaks, top alignment,
insets, baseline = top + largest size, and 1.2 × size line spacing. Left/center/
right alignment now uses shaped advances, including ligatures and positioning.
It adds no wrapping, reflow, autofit, bullets, rotation, or vertical text. Bidi
analysis across differently styled runs/whole paragraphs is deferred. Historical
parser overflow warnings use legacy metrics; configured layout also reports
overflow using actual shaped advances.

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
