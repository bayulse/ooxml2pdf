# PPTX slide inheritance — Milestone 1I-B

Baseline: committed `578453b`. Inheritance is an additive PPTX semantic stage:

```text
OPC relationships -> slide -> slideLayout -> slideMaster -> theme
                 -> sparse placeholder properties -> effective Slide
                 -> existing generic layout / DisplayList -> o2p-pdf
```

No ZIP implementation, XML node, or PDF operation crosses the semantic boundary.
There are no dependency changes. Metadata-only parsing and `inspect` keep all
ancestor and theme XML opaque. Slide placeholders with complete local geometry
now render even when no layout exists; earlier geometry/style omission warnings
are replaced with precise missing-link/unmatched-placeholder diagnostics.

## Discovery and ownership

`relationships(Some(source))` and `part(target)` resolve canonical OPC targets;
no conventional filenames are assumed. Transitional and Strict namespaces and
relationship URIs are supported. Each slide follows at most one `slideLayout`
link; each layout follows at most one `slideMaster` link. Discovered links must
be internal and have the corresponding PresentationML content type. Duplicate,
external, or wrong-type links return `InvalidSlideRelationship`; malformed XML
and supported values retain `InvalidXml`/`InvalidSlide`. Dangling targets are
already rejected by `o2p-ooxml`. Missing links are recoverable and produce
`UnsupportedSlideContent { slide_id, part, name }`; available local properties
still work. This adds a missing-layout diagnostic to earlier synthetic decks.

Traversal has fixed depth; it does not recurse through master-to-layout back
links or scan unrelated parts. Byte, decoded XML, node and depth limits apply
independently to every ancestor part through the existing XML adapter. Shared
masters/layouts are parsed once per presentation; image bytes retain the existing
shared resource cache. Source warnings carry the source part and first referencing
slide ID. Missing slide-link warnings follow that slide's content diagnostics.

`Presentation::slide_layouts()` and `slide_masters()` expose owned records sorted
by canonical part name. They retain background paint, decorative shapes and
placeholder identity/sparse properties. `Slide::placeholders()` retains effective
slide placeholder records in slide XML order. `PlaceholderGeometry` and
`PlaceholderTextDefaults` contain only owned values; `idx()` preserves omission
as `None`, while matching uses the schema default zero. Kind strings preserve
unknown placeholder types. Source getters remain valid after package drop.

## Matching

Only `p:ph` shapes participate. Ordinary shapes and their positions never match.

1. Slide -> layout: match by idx, treating omitted idx as zero. A single idx
   candidate wins. An explicit unmatched idx does not fall back to type.
2. If a slide idx is omitted and there is no zero-idx candidate, a single exact
   type candidate is accepted as a conservative compatibility fallback.
3. Layout -> master: match by compatible type, independent of idx. `ctrTitle`
   maps to `title`; `subTitle`, `obj`, `object`, and `text` map to `body`.
   Other kinds match exactly. This handles ordinary title/body master templates.
4. Multiple candidates warn and produce no match. Unmatched placeholders warn
   and use available local/effective properties; unresolved geometry omits the
   element with a warning.

`obj` is the OOXML default object kind. The descriptive `object` and `text`
spellings are accepted conservatively. Other kinds (date, footer, slide number,
picture, chart, etc.) retain their identity and warn; specialized content is
never synthesized. Placeholder pictures remain unsupported and warn/omit.

## Precedence and fallbacks

For each supported property, the order is **slide local -> matching layout ->
matching master -> existing documented fallback**. Layouts retain effective
master defaults before slides are resolved. Present `false`, zero dimensions,
`noFill`, or unsupported explicit paint are never mistaken for absence.

| Property | Merge / fallback |
| --- | --- |
| x, y, width, height | Independently inherit missing components; no guessed coordinates or extents; omit incomplete geometry |
| rotation / flips | Independently inherit supported values; otherwise zero / false; use existing element-center transform |
| primitive geometry | Local preset -> inherited preset; absent preset allows rectangular textbox paint; unsupported geometry warns/omits |
| fill | Local declaration -> ancestors; explicit noFill blocks ancestors; unavailable explicit color omits paint |
| outline | Width and color inherit independently; noFill blocks outline; default width 0.75pt only for a resolved solid color; zero width omits |
| font request | Local Latin family/token -> ancestors -> configured generic sans-serif |
| font size | Local -> ancestors -> 18pt |
| color | Local supported direct/theme color -> ancestors -> black; unavailable explicit ancestor default color uses black |
| bold / italic | Local -> ancestors -> false |
| paragraph alignment | Local -> ancestors -> left |

The existing local run/paragraph color fallback policy remains unchanged:
unsupported explicit run colors warn and retain their paragraph/default color.
Theme references at all sources use the same `theme_xml` adapter and `Theme`
resolver. A master theme takes priority over the presentation fallback; an
explicit layout theme is scoped to that layout and its slides. Source theme
getters expose the effective resource, while `Presentation::theme()` retains
its presentation-resource contract. Theme overrides/color maps remain unsupported.

Supported ancestor defaults are the first paragraph's `pPr/defRPr`, plus
`lstStyle/defPPr` and `lvl1pPr` (first-paragraph properties override lvl1, which
overrides defPPr). Slide paragraphs keep their own pPr/defRPr and run rPr
precedence over inherited defaults. A slide's first paragraph formatting does
not leak into its later paragraphs. Ancestor sample run formatting and literal
text are not inherited. Multiple ancestor paragraphs warn; only the first one's
defaults are used. Paragraph levels above zero warn and use generic defaults
with local direct properties. This is not the complete txStyles/list hierarchy.

Only the listed properties inherit. Ancestor bodyPr insets/writing mode,
bullets, spacing, effects, style references and complex semantics warn and remain
unsupported. Local/default insets, wrapping, Unicode shaping, font substitution,
embedding and ToUnicode/ActualText continue through the existing path.

## Background and visual layers

The effective background is selected by the **presence** of `p:bg`: slide ->
layout -> master. Supported direct solid backgrounds resolve through the common
color adapter. Explicit noFill stops background inheritance. An explicit bgRef
or unsupported fill warns and leaves paint unavailable; it does not reveal an
ancestor background by accident. bgRef and bgFillStyleLst await Milestone 1I-C.

Painting is deterministic:

1. Effective background (the master background when neither descendant supplies one).
2. Non-placeholder master decorative shapes.
3. Non-placeholder layout decorative shapes.
4. Slide shapes and slide placeholder content.

Within every tree XML order is retained, including fill/outline/text sequencing
and element transform scopes. Rectangles, ellipses, lines, embedded PNG/JPEG and
basic decorative text use the completed shape parser and existing renderers.
Layout/master placeholders are **property donors only**, even when unmatched;
their prompt/sample text and geometry are not copied into decorative layers.
Only actual slide placeholder content renders. This prevents duplicate text.

Slide or layout `showMasterSp=false` suppresses master decorations only; it
does not remove background inheritance or placeholder properties. Layout
decorations remain a separate visible layer. `cNvPr/hidden=true` suppresses
the owning slide/decorative visual element; false stays visible. Hidden ancestor
placeholders still supply properties. `show` retains the previous policy of
including hidden slides. `showMasterPhAnim` warns; animations remain unsupported.

These are intentionally basic layering semantics. Header/footer/date/slide-number
fields, layout-switch behavior, special placeholder content, full master text
styles, theme override/color-map behavior, style-matrix fill/line/effect/background
lists, grouped shapes, charts, SmartArt, notes/comments and animations remain
deferred. DOCX, XLSX and Flutter are outside scope.

## Validation and demonstration

Fourteen minimal synthetic fixtures are regenerated with:

```sh
cargo run --offline -p o2p-pptx --example generate_fixtures
cargo run --offline -p o2p-cli -- convert fixtures/pptx/layout-master-inheritance.pptx --font-dir fixtures/fonts -o target/layout-master-inheritance.pdf
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

The demonstration uses a master background, master strip, layout ellipse,
inherited title geometry/theme font/color, and a body with local position, font-size
and color overrides. Tests inspect owned models, DisplayList order, independent
PDF operators/text, embedded fonts/Unicode mappings, malformed links, warnings,
and all prior rendering regressions. Existing no-warning assertions permit only
the newly required missing-layout warning; rendering checks remain intact.

Reference: [Microsoft's slide master documentation](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-masters)
and [slide layout visibility documentation](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-layouts).
