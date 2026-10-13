# PPTX inheritance — Fidelity 2A

Baseline: clean committed `d1a27f6`. Fidelity 2A extends the structural inheritance
introduced in 1I-B and format references introduced in 1I-C. No dependency changes.

```text
slide -> slideLayout -> slideMaster -> theme
      -> sparse effective properties -> normalized Slide
      -> generic font shaping/layout -> DisplayList -> PDF
```

OPC targets determine ancestry; conventional filenames are never assumed.
Transitional and Strict namespaces share one bounded XML adapter. Metadata-only
parsing and `inspect` leave ancestor/theme content opaque. Missing links and
unmatched/ambiguous placeholders warn; malformed supported values return owned
errors. No parser implementation types or OOXML semantics enter the generic
layout, fonts, or PDF crates.

## Placeholder identity and structural properties

Slide-to-layout matching uses `idx` (schema default zero), with a conservative
unique-kind fallback only when idx was omitted. Explicit unmatched indices never
fall back to another placeholder. Layout-to-master matching uses compatible
kind: title/ctrTitle match title; subTitle/obj/object/text match body for structural
matching. Multiple candidates warn and do not guess. Kind is retained separately
from geometry and content. Specialized picture/chart/footer/date/slide-number
content is not synthesized.

Geometry components, rotation/flips, preset, fills, and outline width/color
inherit individually from slide to layout to master. Complete local geometry
can replace an unsupported ancestor. Incomplete effective geometry omits the
shape with a warning. Explicit false, zero, noFill, and unavailable explicit paint
are present values, not permission to inherit another value. Direct formatting
across the chain precedes separately inherited format references. See
[STYLES.md](STYLES.md).

Background selection uses declaration presence: slide -> layout -> master.
An explicit unsupported/noFill background blocks ancestor backgrounds. Paint order
is effective background, master decorations, layout decorations, then slide
content in XML order. Ancestor placeholder prompt text is never painted.
`showMasterSp=false` on slide or layout suppresses master decorations, retaining
background/placeholder inheritance. `cNvPr/hidden=true` hides a visual but does
not remove a donor's properties. Existing element transforms enclose paint and
text together.

## Effective text properties

`TextListStyle` owns nine sparse property records, one for each zero-based
paragraph level 0..8 (`a:lvl1pPr`..`a:lvl9pPr`). `MasterTextStyles` owns
`p:titleStyle`, `p:bodyStyle`, and `p:otherStyle` from master `p:txStyles`.
`SlideMaster::text_styles()` exposes these owned values; `level()` rejects an
out-of-range index. Family request, size, bold, italic, color, and alignment
remain separate optional properties. Present unresolved family/color values
block inheritance; present false overrides inherited true.

Master style selection uses the slide placeholder role: title/ctrTitle use
titleStyle; non-title content roles (including subTitle/body/obj/object/text) use
bodyStyle; footer/date/slide-number roles and ordinary non-placeholder text use
otherStyle. Structural matching aliases and text-style role selection are
separate decisions. A paragraph's local
`pPr/@lvl` selects the level, defaulting to zero. A missing level does not borrow
level one's formatting. Invalid levels and duplicate level nodes return errors.
Within each list source, a selected level overrides `defPPr` per property.

For a supported paragraph/run, property precedence is:

1. Run-local `rPr` (character properties only).
2. Paragraph-local `pPr/defRPr` and `pPr/@algn`.
3. Shape-local `lstStyle`, selected level before its `defPPr`.
4. Matching layout placeholder's paragraph/list defaults at the selected level.
5. Matching master placeholder's paragraph/list defaults at that level.
6. Nearest effective shape `fontRef` defaults, merged separately from direct
   formatting: slide before layout before master.
7. Applicable master text style at the selected level.
8. Documented property fallback or schema default.

At an ancestor placeholder, paragraph `pPr/defRPr` overrides that ancestor's
list defaults for the paragraph's level. If multiple ancestor paragraphs specify
the same level, the first supplies its defaults and a warning describes the
ambiguity. Ancestor literal text and sample run `rPr` are not inherited.
Slide-local paragraph defaults never leak into another paragraph.
`endParaRPr` applies to the empty/end paragraph style, not to preceding runs.
Local list default properties override ancestor level properties; resolution
never chooses one complete style object at the expense of another source's
individual properties.

Latin `+mj-lt` and `+mn-lt` references resolve through the effective theme to a
real major/minor family request. Shape `fontRef major/minor` uses the same theme
resolver. The generic font library still handles availability, substitution,
Unicode shaping, font fallback, and embedding; successful OOXML family resolution
does not imply that the requested font was registered.

Textbox `bodyPr/@lIns,rIns,tIns,bIns` inherit individually through placeholders.
Local values (including zero) override layout then master values. Absent values
use 7.2pt left/right and 3.6pt top/bottom. Non-top vertical anchoring, inherited
writing modes, unsupported wrapping, and autofit retain warnings; this milestone
does not implement those behaviors. Vertical **insets** are supported, vertical
writing and middle/bottom text anchoring are not.

## Fallback diagnostics

After effective resolution, one `UnsupportedSlideContent` text inheritance/font
warning per affected textbox names unresolved family, size, and/or color. It is
no longer emitted merely because a shape is not a placeholder. Complete local
formatting and complete inherited formatting both avoid this warning.
Unresolved font size uses 18pt; family uses generic sans-serif; color uses black.
An unsupported explicit run color retains its paragraph/default color and warns.
Bold/italic false and left/top alignment are supported schema/default behavior.
Missing fonts still produce separate font-subsystem warnings. This is not a
suppression of font substitution, overflow, or unsupported-typography warnings.

The remaining public-corpus fallback is `004-theme-layout`, slide 1's body
placeholder: its local size/family resolve, but no ancestor or master style
supplies text color. It now receives an unresolved-color diagnostic.

## Color-map scope and source caching

See [THEMES.md](THEMES.md) for the owned ColorMap and clrMapOvr rules. Every
supported paint path uses the consuming slide's effective map. Cached master
and layout records retain source-scoped maps/themes. Because their existing
normalized paints are concrete RGB, an affected ancestor is re-evaluated through
the existing parser when a slide override or layout theme changes its scope.
This bounded re-evaluation avoids mutating shared caches, duplicating placeholder
prompt text, or changing the theme ColorScheme. Repeated shared-source diagnostics
are reported once per source part/feature. Raster assets remain shared.

## Remaining typography scope and validation

Presentation `defaultTextStyle`, theme overrides, script-specific theme font
selection, bullets/numbering, margin/indent/tab layout, paragraph/line spacing,
kerning/character spacing, underline/strike, autofit, middle/bottom anchoring,
vertical writing, independent bodyPr rotation, and advanced effects remain
unsupported and warn when encountered. Supporting list levels selects text
properties; it does not enable bullet rendering. Unknown properties remain
visible in structured warnings. Charts, SmartArt, groups, gradients, rounded
rectangles, animations, DOCX, XLSX, and Flutter remain outside this milestone.

Ten text and nine color-map fixtures supplement all existing regressions.
Tests cover independent precedence, all nine levels, explicit false/zero,
missing levels/fallbacks, malformed input, Strict namespaces, theme families,
color-map scope, Unicode PDF extraction, ToUnicode, and deterministic output.
Existing wrapping/Unicode/rotation/flip tests remain in place.

```sh
cargo run --offline -p o2p-pptx --example generate_fixtures
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

Semantic sources: [Microsoft master text styles](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.presentation.textstyles)
and [Microsoft slide master/color-map semantics](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-masters).

Placeholder-role selection is also cross-checked against the primary
[Apache POI PresentationML implementation](https://github.com/apache/poi/blob/trunk/poi-ooxml/src/main/java/org/apache/poi/xslf/usermodel/XSLFTextParagraph.java)
(`getDefaultMasterStyle`); this establishes subtitle/body selection without
inventing a style from the placeholder's name.
