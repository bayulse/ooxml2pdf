# Rounded geometry and outer shadows — Fidelity 2B

## Boundaries and representation

`o2p-pptx` interprets DrawingML presets, adjustment guides, effect references,
colors and units. `o2p-core` contains only generic, positioned geometry:
`RoundedRectOp` and `StrokeRoundedRectOp` specify a rectangle, circular corner
radius, paint and optional outline width. `ShadowOp` contains a rectangle,
rounded rectangle, ellipse or image silhouette, fill/stroke coverage, and a
`Shadow` with local point offset, blur radius, RGB color and opacity in [0, 1].
`Shape::Shadowed` owns an element. The adapter emits its shadow immediately
before that element, inside the same element transform scope. Text follows the
owning geometry; later elements retain their original order. There is no text
shadow or global shadow pass. `Shape::RoundedRectangle` retains fill, no-fill,
outline and text behavior. The PDF backend has no Office preset names, units,
XML nodes or theme indices.

## Rounded rectangle semantics

DrawingML `roundRect` uses the preset guide `adj`, default 16667, clamped to
[0, 50000]. Corner radius is `min(width, height) * adj / 100000` in points.
A single `a:gd name="adj" fmla="val INTEGER"` in `a:avLst` overrides the default,
including zero and values that clamp at the preset endpoints. This is independent
of slide size, raster DPI and corpus pixels. Unsupported guide names/formulas
produce source-scoped structured warnings and retain the preset default;
malformed integers and duplicate supported guides produce owned parse errors.
The same semantics apply to ordinary shapes and inherited placeholders.

Each rounded rectangle PDF path contains four straight segments and four cubic
quarter-circle approximations (control factor 0.5522848). Fill and outline
remain vector operations, including under rotation and horizontal/vertical
reflection. Radius is validated at the PDF boundary even for caller-created
DisplayLists. General preset geometry and guide formula evaluation are deferred.

## DrawingML outer shadows and precedence

Supported `a:effectLst/a:outerShdw` properties are `blurRad`, `dist`, `dir`, color,
`a:alpha`, and `rotWithShape`. Missing blur/distance/direction default to zero;
opacity defaults to one and rotation defaults to true. Both dimensions use the
shared EMU conversion (12700 EMUs per point). Direction uses the shared clockwise
DrawingML angle conversion (60000 units per degree), then
`dx = distance * cos(angle)`, `dy = distance * sin(angle)` in y-down coordinates.
Direction must be within [0, 21600000). Blur and distance are bounded unsigned
32-bit EMU values; larger coordinates return a structured parse error. With `rotWithShape="0"`, the offset is
converted into element coordinates using the inverse orthogonal transform, so
its eventual direction remains page-relative while its silhouette follows the
owning element.

Colors use the existing centralized color-expression parser/resolver, effective
color map and theme, including `srgbClr`, `schemeClr`, `phClr` reference color,
system `lastClr`, tint/shade/luminance transforms and shadow alpha [0, 100000].
Alpha support is opt-in for effect colors; this milestone does not change the
supported alpha behavior of ordinary fills or text. It introduces no second
color subsystem.

The theme's `effectStyleLst` retains every entry's position; `effectRef idx=0`
explicitly selects no effect, and positive indices select entry `idx-1`.
`StyleMatrix::effect_style_count()` exposes the number of entries without
exposing deferred/XML types. Shape/picture style references resolve only outer
shadows. Direct local effect lists take precedence over inherited effects and
style references. Explicit empty effect lists suppress inherited/theme effects.
Unsupported local effect containers also suppress fallback theme effects.
Placeholder direct effects and style references follow the existing per-property
slide/layout/master inheritance. Malformed supported definitions return owned
errors; missing/out-of-range references warn and omit their effects.

Default scale/skew values (`sx=sy=100000`, `kx=ky=0`) are supported. Nondefault
scale/skew warns and omits the shadow. Alignment is validated; it does not affect
this unscaled/unskewed subset. Inner shadow, glow, bevel, reflection, effect DAGs,
3D, line and text shadows remain unsupported and diagnosed. Unsupported sibling theme
effects do not prevent a supported outer shadow in the same effect style.

## PDF blur strategy and security

PDF 1.7 paths do not have a native blur operator. The backend creates only the
individual shadow's local RGBA bitmap and embeds it as an image XObject with a
DeviceGray soft mask through the existing bounded image adapter. The original
shape, its outline and its text remain vector. Image shadows sample the source
image's alpha, so transparent areas do not cast rectangular shadows.

The silhouette is sampled at four subpixel positions. Three separable box-blur
passes approximate a Gaussian with sigma `blur_radius / 2`; fractional box radii
blend adjacent kernels. This is an explicitly approximate blur, not a claim of
pixel-identical PowerPoint convolution. Running sums make blur work linear in
pixel count rather than proportional to blur radius. Padding is 1.5 blur radii
plus half the stroke width and one point. Opacity is applied to the final mask.

Target resolution is 144 DPI (two pixels per point), reduced adaptively to obey
both hard limits: **2048 pixels on either side and 1048576 total pixels per
shadow**. Integer rounding cannot exceed either limit. Giant shapes/blur values
therefore reduce local resolution rather than allocate a giant slide bitmap.
A mask and scratch buffer consume at most 8 MiB; RGBA storage adds at most 4 MiB.
Image decoding, PNG encoding/compression and PDF serialization have additional
buffers. Source images retain the existing 16-megapixel, 16384-dimension and
128-MiB decoder limits. Source decode buffers are dropped before blur passes.
Scratch buffers are released per shadow; encoded local images and PDF bytes
remain proportional to document content. These are per-shadow limits, not a
constant-memory promise for arbitrarily many elements/pages.

Nonfinite parameters, negative blur/stroke, invalid opacity/radius, overflowing
extents and malformed images produce owned errors before mask allocation.
Dimensions and extents use wider intermediates. No dependency, runtime service,
external process or large rendering engine was added. Existing `image`, `flate2`
and `pdf-writer` adapters perform encoding and PDF serialization.

## Validation

Eleven deterministic PPTX fixtures cover default/custom corners, text, transforms,
all four shadow silhouettes, theme effects, local precedence and overlapping
paint order. Parser/backend regressions check conversions, color/alpha, explicit
empty effects, malformed/extreme inputs, vector PDF operators, soft masks,
transparent image silhouettes and actual raster allocation limits.
See [the Fidelity 2B report](../compat/reports/FIDELITY_2B.md) for the source/reference
identity mismatch, both six-case measurements, warning changes and visual review.

Specification references: [Microsoft's outer-shadow documentation](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outershadow?view=openxml-3.0.1)
and [blur-radius coordinate semantics](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.outershadow.blurradius?view=openxml-3.0.1).
