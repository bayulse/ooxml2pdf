# DrawingML style matrices — Milestone 1I-C

Baseline: committed `336f4e2` (Milestone 1I-B). This is an additive PPTX semantic
stage. No dependency changes or changes to OPC, generic font/layout handling,
the DisplayList contract, or PDF serialization are needed.

```text
PPTX style references -> scoped theme fmtScheme -> effective paint/font defaults
                     -> existing Slide / Shape / TextStyle -> DisplayList -> PDF
```

## Owned model and parsing

`Theme::styles` owns a `StyleMatrix`. Its three ordered collections correspond
to `fillStyleLst`, `lnStyleLst`, and `bgFillStyleLst`. Public count methods allow
inspection; entry representations, indices, deferred color expressions and XML
types remain private. A fill is no fill, solid color, or an unsupported marker.
A line retains optional validated width in EMUs and its fill. Color expressions
hold a concrete RGB color, scheme slot (including `phClr`), or unavailable color,
plus validated integer transform values. They contain no XML nodes or PDF types.

Unsupported entries retain their positions. Removing a gradient entry would
incorrectly shift later referenced styles. Empty or missing format lists are
accepted for minimal packages; selecting an absent entry warns. A missing
`fmtScheme` gives empty lists. This is a bounded subset parser, not full OOXML
schema validation. Duplicate supported lists, multiple colors/fills, malformed
supported widths/transforms and unexpected structural text return
`InvalidTheme` or `InvalidSlide`; malformed XML/security limits retain existing
errors. The existing per-part XML byte/node/depth limits apply to every theme.
Metadata-only parsing and `inspect` still leave theme contents opaque.

Empty `effectStyleLst` and empty effect containers carry no effects. Nonempty
advanced effect content warns at its boundary; no effect matrix is implemented.
An `effectRef` of zero means no effect. Other/malformed effect references warn
and omit the effect. Independent supported fill/line/font components still work.

## Index mapping

Indices are unsigned 32-bit DrawingML values, with surrounding whitespace and
an optional leading plus accepted. Numeric list positions are **one-based**.

| Reference | idx | Implemented meaning |
| --- | --- | --- |
| fillRef or bgRef | 0 or 1000 | Explicit no fill; no theme required |
| fillRef or bgRef | 1–999 | `fillStyleLst[idx - 1]` |
| fillRef or bgRef | 1001 and above | `bgFillStyleLst[idx - 1001]` |
| lnRef | 0 | No referenced line paint |
| lnRef | 1 and above | `lnStyleLst[idx - 1]`; never background-fill indexing |
| fontRef | major / minor | Existing major / minor Latin theme font role |
| fontRef | none | Generic font request; no theme font role |

Thus 1001 selects the *first* background-fill entry, while 1000 selects no fill.
Both shape fills and backgrounds can select either fill collection. Line 1001
is a line-list selection and normally out of range. Never clamp an invalid
index or replace it with the first/last style.

Missing, negative, nonnumeric, overflowing and out-of-range indices produce
source-scoped structured `PptxWarning::UnsupportedSlideContent` values with
slide ID, canonical source part, reference name/index, and fallback description.
These recoverable reference failures omit the referenced paint; malformed
matrix definitions remain errors. Unknown font roles warn and request generic
sans-serif. Unknown reference children and unsupported attributes warn too.

References: [Microsoft FillReference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fillreference?view=openxml-3.0.1),
[BackgroundStyleReference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.presentation.backgroundstylereference?view=openxml-3.0.1),
[LineDefault](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.linedefault?view=openxml-2.10.0),
and [FontReference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fontreference?view=openxml-3.0.1).

## Precedence and inheritance

For each supported property:

1. Direct/local formatting.
2. Direct formatting from matching ancestor placeholders (layout before master).
3. Referenced theme defaults (nearest slide reference, then layout, then master).
4. Existing fallback.

Direct properties and referenced defaults remain separate through the ancestor
merge. Resolving a master reference into its direct properties too early would
make it override a later local reference, or a local reference override an
inherited direct value. Width and color merge independently; a local width can
use referenced color and a local color can use referenced width. A complete
local line overrides both. Explicit `noFill`, zero width, false values and
unsupported explicit paint never become absent properties.

A present unavailable reference blocks an older reference for that component,
but cannot override any direct value. Missing style components inherit
independently; a shape need not supply fillRef, lnRef and fontRef together.
References are diagnosed even when explicit formatting makes them unnecessary.
The sparse placeholder matching rules and source visibility/layering are
unchanged. Ancestor prompt text still does not render. Placeholder text-default
getters expose the effective defaults while internal direct values remain sparse.

Background selection still uses **presence of p:bg**: slide > layout > master.
An explicit no-fill, unsupported or unavailable background blocks ancestors.
`bgRef` uses the selected source's scoped theme. If a nonconforming background
contains both bgPr and bgRef, bgPr wins; bgRef is still parsed/diagnosed.

## Supported paint, colors and fonts

Solid and no-fill styles work for rectangles, ellipses, simple lines and
text-bearing shapes, including rectangular text-only paint. Both ordinary and
placeholder shapes use the same style-reference adapter. PresentationML
`p:style` (with DrawingML children) is the normal shape container; DrawingML
`a:style` is accepted through the same adapter. Strict and Transitional
namespaces use the same semantics.

Solid lines support color, width (0–20,116,800 EMUs), no fill, flat caps, single
compound mode, centered alignment and solid dash declarations. A resolved
color without width uses the existing 0.75pt default. Width zero produces no
PDF hairline. lnRef zero supplies no color; direct color can still render with
the default width. Unsupported line decorations, joins, caps, dashes, arrows or
complex fills warn and omit the referenced stroke. Local unsupported line
semantics also block referenced paint.

All color parsing/resolution uses the existing central `theme_xml` adapter,
`Theme::resolve_color`, and shared transform math. Matrix entries may contain
sRGB, system `lastClr`, concrete scheme slots, and supported tint/shade/
lumMod/lumOff transforms. `phClr` substitutes the reference's resolved color,
then applies the entry's own transforms. Reference transforms therefore run
before matrix transforms. A fixed matrix color does not require a reference
color. Missing phClr substitution warns and makes that color unavailable.
No Office default palette, OS color lookup, or color-map alias is guessed.

`fontRef major/minor` maps to the existing `+mj-lt/+mn-lt` resolver. Its color
supplies a text default when present. Local run/paragraph/list defaults and
inherited direct font/color defaults retain priority. Actual family requests
continue through generic substitution, shaping, wrapping and embedded fonts;
no PPTX role/index enters `o2p-fonts`. Missing fonts retain existing font warnings.
The unconfigured compatibility path still uses standard PDF fonts. Full txStyles,
script-dependent role selection and PowerPoint text hierarchy are deferred.

## Unsupported fidelity and validation

Gradient, pattern, image/group and unknown matrix fills are never approximated.
They warn at theme parsing and again when referenced, then omit affected paint.
Unsupported color transforms (including alpha) warn and make the color
unavailable. Text keeps its existing paragraph/default color fallback. Supported
direct formatting survives unsupported references. Generic drawing order,
transforms, raster assets, wrapping, font embedding and Unicode mappings remain
unchanged. Existing legacy unwrapped-text overflow warnings may still appear
even when configured-font wrapping fits; this milestone preserves that contract.

No advanced gradients, effects, arrows, groups, charts, SmartArt, animations,
DOCX, XLSX or Flutter integration are implemented. Color maps/theme overrides
and complete PowerPoint formatting fidelity remain deferred.

Twelve original fixtures cover fill/line references, local overrides, master/
layout sources, backgrounds, font roles, gradients, malformed indices and a
visible combined demo. Tests cover split indexing/sentinels, sparse precedence,
source themes, Strict XML, unsupported entry positions, transformations/images,
fonts/Unicode, PDF operators/determinism and existing security/regressions.

```sh
cargo run --offline -p o2p-pptx --example generate_fixtures
cargo run --offline -p o2p-cli -- convert fixtures/pptx/style-matrix-demo.pptx --font-dir fixtures/fonts -o target/style-matrix-demo.pdf
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

## Fidelity 2A integration

Color-map semantics and the supported master text-style hierarchy now precede
final fallback. All scheme references, including matrix-entry expressions, use
the effective map through the central color adapter. Direct/inherited properties
still precede separately merged references; references precede applicable master
text styles. See [THEMES.md](THEMES.md) and [INHERITANCE.md](INHERITANCE.md).
The earlier deferred-text/color-map statements above describe 1I-C history.
