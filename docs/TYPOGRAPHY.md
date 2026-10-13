# Paragraph typography — Fidelity 2C

Baseline: clean committed `2b73b7b`. Runtime remains native Rust and offline.
No dependencies changed. This document describes current paragraph behavior;
earlier milestone descriptions in FONTS.md and INHERITANCE.md are historical.

## Semantics and ownership

`o2p-pptx` resolves DrawingML properties into effective owned Paragraph/TextBox
values. It retains sparse property presence during inheritance, including
explicit zero, false, no-bullet and follow-text-font values. It never emits PDF
commands. `o2p-core` owns generic `Bullet`, `FontRequest`, `ParagraphFormat`,
`ParagraphSpacing` and `VerticalAlignment`. `o2p-fonts::FontRequest` remains a
public re-export of the same owned request, preserving existing imports.
`o2p-layout` measures/wraps/positions text; `o2p-fonts` resolves and shapes;
`o2p-pdf` serializes positioned operations without paragraph/Office semantics.

Configured-font and legacy fixed-font conversion share the same effective
paragraph adapter. Both delegate indentation, spacing and vertical positioning
to `o2p-layout`. The legacy path retains explicit breaks and fixed standard-font
advances; its reporting renderer warns/replaces unrepresentable Unicode. The
configured path produces embedded, selectable GlyphText operations for bullets
and text, retaining source Unicode, ToUnicode/ActualText and font diagnostics.

## Bullets and geometry

Character bullets preserve the bounded Unicode `a:buChar/@char` string, including
custom characters and combining sequences. `a:buFont/@typeface` becomes a generic
font request; Latin theme aliases resolve through the existing theme adapter.
Absent bullet font follows the first text run's effective family/style/size/color
(end paragraph style for an empty paragraph). Explicit `buFontTx` blocks inherited
bullet font selection. Availability, substitutions, glyph coverage and missing
glyphs are handled by the existing generic font system, without built-in Office
aliases or symbol-character remapping. Empty paragraphs occupy a line but do not
paint a bullet. Bullet glyphs share the first text line's baseline.

`buNone` explicitly disables inherited bullets. Unsupported numbering/image
bullets warn and block an older character bullet rather than silently turning
numbering into a character list. Contradictory bullet/font declarations fail
with an owned error. Custom bullet color/size properties remain unsupported and
warn; character bullets currently follow text size and color.

DrawingML `marL`, `marR` and signed `indent` convert EMUs to points (12,700 per
point). They inherit independently. If no source defines them, PowerPoint/Office
uses zero for marL and indent, rather than the different defaults implied by the
base standard; see [Microsoft's implementation notes](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/9b34280e-538e-4811-8af9-761d34f88f20).
This fallback is applied only after inheritance. Relative to the already-inset rectangle:

- Text origin for a bullet paragraph: inner left + left margin.
- Bullet origin: text origin + signed indent. A negative indent hangs left.
- Continuation text: inner left + left margin, including after explicit breaks.
- Nonbullet first line: inner left + left margin + signed indent.
- Available width: inner width minus the applicable text offset and right margin,
  clamped to zero. Oversized clusters still make bounded forward progress.

Left/center/right alignment uses each line's available width and shaped advance.
The bullet moves with the first line's alignment offset and does not consume
text width or introduce a fixed margin. Existing cluster-safe whitespace wrapping
and line-edge reshaping remain unchanged.

## Paragraph spacing and textbox anchoring

`spcBef/spcAft` support `spcPts` (hundredths of a point) and `spcPct` (100,000 =
100%, or a decimal percent string). Percentages become generic text-size multiples
and use the largest font on the first/last line respectively. Spacing adds to
paragraph boxes, never to the line count. Fidelity 2E uses scalable font line metrics, described below. Percentages still
use the largest text font size on the first/last line, independently of line
height or bullet size. Explicit `lnSpc` remains unsupported and warns.

`bodyPr@spcFirstLastPara` defaults to false: omit the first paragraph's before
spacing and the last paragraph's after spacing at textbox edges. An inherited or
explicit true includes them. Interior paragraphs preserve both spacing values.
Recognized unsupported spacing forms produce source-scoped structured warnings
and a present zero fallback, which blocks older spacing. Malformed values,
duplicates, nonfinite percentages and out-of-range units return owned errors.

`bodyPr@anchor=t/ctr/b` becomes Top/Center/Bottom. Resolve inherited insets first,
then lay out all paragraphs and spacing to determine block height. Start at inner
top, inner top + (inner height − block height)/2, or inner bottom − block height.
Overflow retains every line and warns, including for centered/bottom overflow.
Position locally before applying the existing shape rotation/flip scope. Anchor
does not rotate text independently or alter the shape's center.

## Inheritance

Direct paragraph > slide-local list level/default > layout placeholder paragraph
and list level/default > master placeholder paragraph and list level/default >
applicable master title/body/other text style > fallback. Existing font-reference
resolution remains between direct ancestor properties and master text styles.
Each of the nine paragraph levels resolves independently. A local bullet glyph
can retain an inherited bullet family, indentation, alignment and spacing.
Local no-bullet and explicit zero do not disappear during sparse merges.

Anchor, edge-spacing policy and each inset resolve independently through slide
bodyPr > matching layout bodyPr > matching master bodyPr > schema/default.
Supported effective anchors no longer emit the top-alignment fallback warning.
Unsupported writing modes, independent bodyPr rotation and unsupported anchors
continue to warn.

## Evidence and deferred autofit

004-theme-layout slide 2 has empty local paragraph defaults/list styles. Its layout
adds level selection but no bullet properties. The body master style supplies
Arial `•`, marL=342900, indent=-342900, left alignment and 20% before spacing.
The title master style supplies centered paragraph alignment and its master
placeholder supplies centered vertical anchor. Earlier output retained text
defaults/alignment but discarded bullets, margins, spacing and vertical anchor.

Across the six current paired PowerPoint ZIPs, master styles use 60 spcBef values
(six zero and 54 at 20%); ordinary text uses 159 spcAft values of 400 (4pt).
There are 156 `spAutoFit` and 12 `normAutofit` declarations across all package
parts, including unused layout placeholders. `spAutoFit` requests adapting shape
size to text; `normAutofit` can adapt text sizing/line spacing. Neither is applied
in 2C: authored geometry and font sizes remain fixed and structured warnings are
retained. No heuristic shrinking, iterative reflow or shape resizing is performed.

Fifteen reproducible synthetic fixtures and generic/parser/PDF tests cover
characters/fonts, overrides, inheritance, hanging/wrapped text, points/percentages,
multiple paragraphs, title/body alignment, insets and rotated anchoring. Paired
measurements and native-resolution visual findings are recorded in
[FIDELITY_2C.md](../compat/reports/FIDELITY_2C.md).

Semantic sources: [Microsoft paragraph properties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.paragraphproperties),
[spacing points](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.spacingpoints),
[spacing percentages](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.spacingpercent),
and [textbox body properties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.bodyproperties).

## Fidelity 2E — Line boxes and explicit baselines

Configured-font layout takes independent maximum ascent, descent and nonnegative
line gap across styled fragments and every resolved fallback face. First-line
bullets participate and share the resulting baseline, without changing text width
or the text size used for percentage spacing. Blank paragraphs and explicit-break
lines use resolved end-paragraph font and size (`Paragraph::empty_font` and
`empty_font_size`).

Coordinates are downward-positive points. For line top T and metrics A, D, G:

- baseline = T + A;
- font-box bottom = baseline + D;
- line bottom = T + A + D + G;
- line height = A + D + G.

Leading follows descent; it is not split around the font box. Glyph origins are
baselines, not line tops. `Line` exposes top, baseline, metrics, bottom, height,
width and largest text size separately. Styles/resources/offsets remain per run.
Ink bounds and combining-mark extents are not line-box metrics.

Block height sums line heights and retained paragraph spacing. Top anchor starts
at inner top; center at `inner_top + (inner_height - block_height) / 2`; bottom at
`inner_top + inner_height - block_height`. Each baseline adds its line ascent.
PPTX resolves insets first; shape transforms apply afterward. No title or font
special case exists. The unconfigured standard-font compatibility path retains
nominal metrics because it has no resolved scalable font resources.

Empty `normAutofit` implies fontScale=100000 (100%) and lnSpcReduction=0 (0%).
It supplies no extra scale or baseline offset in this title. Sources:
[font scale](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit.fontscale)
and [line-space reduction](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit.linespacereduction).
General normAutofit, iterative fitting, spAutoFit and explicit line spacing remain
deferred with existing structured warnings. Kerning, character spacing and
wrapping-width policy are unchanged.

`TextBox::measure_with_fonts` returns the generic layout report before transforms.
The development-only `text_layout_trace` CLI example prints effective geometry,
insets, anchor, selected metrics, block height and line/run origins. Supply the
same explicit font flags as the compatibility adapter. Normal conversion has no
new logging. See [Fidelity 2E evidence](../compat/reports/FIDELITY_2E.md).
