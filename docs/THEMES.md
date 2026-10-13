# PPTX themes and fidelity — Milestones 1I-A / 1I-B / 1I-C

Baseline: `f392e61`. This milestone resolves directly referenced theme colors and
explicit Latin theme font tokens. Theme semantics live entirely in `o2p-pptx`.
No dependency, package-reader, layout, font-subsystem or PDF change is required.

## Discovery and ownership

The chain is package `officeDocument` -> detected presentation part -> internal
`theme` relationship -> DrawingML theme part. Relationship targets are already
canonicalized by `o2p-ooxml`; PPTX calls `relationships()` and `part()`, never ZIP
internals or filename guesses. Both Transitional and Strict relationship/XML
namespaces work, including relocated main/theme parts. The target must declare
`application/vnd.openxmlformats-officedocument.theme+xml`.

Content parsing also follows slide -> layout -> master and loads the master's
explicit theme, falling back to the presentation theme. An explicit layout
theme takes priority for that layout and its slides. All sources use this same
theme loader and color/font resolver; there is no alternate resolution path.
Theme overrides remain unsupported. Unrelated theme parts are ignored. External
themes warn and remain unavailable; nothing is fetched. Multiple presentation
theme relationships or wrong target content types return `InvalidTheme`.
Dangling internal targets remain errors from the existing OPC reader.

`Presentation::parse_with_content()` loads the presentation theme before content,
and caches reachable layout/master semantic models during inheritance traversal.
`Presentation::theme()` exposes an owned optional `Theme`, with `ColorScheme`,
`FontScheme`, `ThemeFonts` and the owned `StyleMatrix` in `Theme::styles`. The model contains concrete core RGB colors,
owned typeface strings and script mappings, never XML nodes or parser types.
It outlives the package. Layout/master `theme()` getters expose their scoped
effective theme; `Presentation::theme()` retains its presentation-only contract.
Metadata-only `parse()`, blank-page conversion and
`inspect` keep theme bytes opaque, including malformed theme XML.

The existing XML adapter enforces byte/decoded-byte, node and depth ceilings
per theme part, UTF-8/UTF-16 encoding checks, DTD rejection and preflight before
recursive parsing. Supported structures require `theme/themeElements`,
`clrScheme`, `fontScheme`, `majorFont/latin` and `minorFont/latin`. Malformed XML
returns `InvalidXml`; malformed supported values/duplicates return
`InvalidTheme`. XML security-limit errors propagate unchanged.

## Colors and priority

`themeElements/clrScheme` supports all twelve concrete slots:

| Slots | Meaning |
| --- | --- |
| `dk1`, `lt1`, `dk2`, `lt2` | Dark/light colors |
| `accent1` … `accent6` | Accent colors |
| `hlink`, `folHlink` | Hyperlink/followed hyperlink colors |

Definitions support six-digit, case-insensitive `srgbClr/@val` and
`sysClr/@lastClr`. The latter is deterministic and never queries OS colors.
Missing `lastClr`, missing scheme slots, and unsupported concrete representations
produce `UnsupportedTheme { part, name }`; those slots stay unavailable.
Missing or invalid required attributes and duplicate slots/definitions fail.
Recursive scheme definitions inside themes are unsupported rather than guessed.

All supported backgrounds, rectangle/ellipse fills, shape outlines, lines and
text colors use one private color adapter and `Theme::resolve_color()`:

1. Explicit `srgbClr` resolves directly, independently of any theme.
2. Explicit `sysClr` resolves through `lastClr` only.
3. Explicit `schemeClr` resolves the named concrete slot in the loaded theme.
4. Supported transforms apply to the resolved base in XML order.
5. An unavailable color omits background/fill/stroke/line paint; text retains its
   local paragraph/default color (black if no local default exists).

There is exactly one color per solid fill; conflicting declarations are errors,
not priority rules between multiple sibling colors. Missing themes have no
invented Office palette. Every unresolved slide reference produces a
slide-scoped `UnsupportedSlideContent` warning describing the fallback.
Explicit font/color values continue to override local defaults.

`bg1`, `bg2`, `tx1` and `tx2` require color-map semantics and remain unresolved.
`phClr` is supported inside style-matrix entries through reference-color
substitution; outside that context it warns and remains unavailable. This milestone does not infer the usual dk/lt mapping
or reinterpret a master color map.

## Conservative color transforms

The shared math is independent of XML and tested separately. Percentages use
DrawingML integer units: 100,000 = 100%. `tint` and `shade` accept 0–100,000;
`lumMod` accepts nonnegative signed-i32 values; `lumOff` accepts signed-i32 values.
Invalid percentages fail. This subset accepts integer lexical values, not
percentage-sign strings. Calculations use f64, clamp after each transform and
round to byte channels once at the end, with deterministic nearest rounding.

| Transform | Implemented calculation (`p = val / 100000`) |
| --- | --- |
| `tint` | Each normalized sRGB channel: `c*p + (1-p)`; 0 is white, 100% unchanged |
| `shade` | Each normalized sRGB channel: `c*p`; 0 is black, 100% unchanged |
| `lumMod` | HSL lightness `L*p`, preserving hue and saturation |
| `lumOff` | HSL lightness `L+p`, preserving hue and saturation |

Consecutive luminance transforms retain HSL hue/saturation even at black/white.
Transforms work on direct colors, theme definitions and scheme references.
This is opaque sRGB/HSL support, not a full color-management/effects engine;
gamma-aware linear-light mixing is not claimed. Unsupported transforms,
including alpha/transparency, saturation, hue, gamma and inversion, warn and
make that color unavailable; they are never silently dropped.

The [ECMA-376 DrawingML primer](https://download.microsoft.com/download/e/1/4/e14fb96f-83b8-4a2a-84db-7fa8acbe061a/Office%20Open%20XML%20Part%203%20-%20Primer.pdf)
describes tint/shade mixing. [Microsoft's luminance documentation](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.luminanceoffset?view=openxml-3.0.1)
specifies preserving hue and saturation for luminance changes.

## Explicit theme font tokens

`fontScheme` captures major/minor Latin, available `ea`/`cs` typefaces and
`font/@script,@typeface` mappings. Empty typefaces remain unavailable. Script
mappings are preserved for future selection, not applied to infer run styles.

Explicit `a:latin/@typeface="+mj-lt"` resolves to major Latin; `+mn-lt` resolves
to minor Latin. This also applies to supported inherited placeholder defaults,
explicit local paragraph defaults and end
properties already supported by the text parser. Resolved family names enter
`TextStyle::family`, then ordinary `FontRequest` -> generic resolver -> shaping
-> measured layout -> embedded PDF fonts. `o2p-fonts` knows no PPTX theme tokens.

If the resolved family is unregistered, it is still requested by its real name;
existing `MissingFont`/`Substitution` diagnostics and configured fallback order
apply. Missing themes/empty Latin names/unsupported aliases (including `+mj-ea`
and `+mn-cs`) warn and use the existing generic sans-serif request. Tokens never
reach the generic resolver as literal family names. Absent run typefaces first
use supported placeholder defaults, then the generic request; major/minor fonts
are not inferred merely from a placeholder's type. The unconfigured standard-Helvetica
compatibility path retains its font-selection limits; supply fonts for fidelity.

## Deferred semantics and validation

Basic master/layout/placeholder geometry, paint, text defaults and backgrounds
are supported by 1I-B; see [INHERITANCE.md](INHERITANCE.md) for precedence and limits.
Milestone 1I-C resolves solid/no-fill format-matrix fills, lines and backgrounds,
and conservative fontRef defaults. See [STYLES.md](STYLES.md) for indexing,
precedence, phClr, warnings and fallbacks. Full txStyles, theme overrides, effect
styles, grouped shapes, charts, SmartArt and animations remain unsupported.
Unsupported theme subtrees warn at their boundary. DOCX, XLSX and Flutter stay
outside scope. Existing shape/image/text z-order, wrapping, transforms, Unicode,
font embedding and ToUnicode/ActualText serialization are preserved.

Ten minimal synthetic fixtures cover accents, outlines/lines, all twelve slots,
text colors, major/minor fonts, transforms, missing/malformed themes and a
complete palette with themed font requests. Regenerate with the existing PPTX
example. Tests cover package relationships, parsing, independent transform math,
fallback diagnostics, hostile inputs, normalized paint, PDF operators/embedding/
Unicode extraction, CLI determinism and failure preserving existing output.

```sh
cargo run --offline -p o2p-cli -- convert fixtures/pptx/theme-colors-and-fonts.pptx --font-dir fixtures/fonts -o target/theme-colors-and-fonts.pdf
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

## Milestone 1I-C integration

`fmtScheme` is now parsed through the existing bounded theme adapter. Matrix
colors retain owned deferred expressions and share the direct-color resolver
and transform math. Supported indices resolve only after the scoped theme and
reference color are known. Direct/inherited placeholder formatting has priority
over references; reference defaults precede generic fallback. `fontRef major`
and `minor` use the existing Latin family resolver. No generic rendering or
font-library dependencies change. See [STYLES.md](STYLES.md).
