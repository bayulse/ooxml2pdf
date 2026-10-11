# Synthetic package fixtures

All documents here are generated specifically for this repository's tests.
They contain no organizational documents, proprietary assets or personal data.
ZIP timestamps are fixed. The one-pixel PNG is generated from original RGBA
bytes by Rust. These are tiny package/parser fixtures, not Office fidelity
reference documents.

| File | Purpose |
| --- | --- |
| `minimal.pptx` | One empty slide, complete OPC metadata and relationships |
| `one-image.pptx` | One slide, one PNG, relative image target, external hyperlink |
| `three-slides.pptx` | Three slide parts with relationships |
| `malformed-relationships.pptx` | Relationship missing required `Type` |
| `malformed-xml.pptx` | Truncated relationship XML |
| `path-traversal.pptx` | ZIP member named `../escape.dat` |
| `target-traversal.pptx` | Package relationship tries to escape the root |
| `entry-limit.pptx` | 21 entries; fails at a configured maximum of 20 |
| `compression-abuse.pptx` | 1 MiB of zeros exceeds the default compression ratio |

Regenerate deterministically with:

```sh
cargo run --offline -p o2p-ooxml --example generate_fixtures
```

Additional damaged ZIPs, limits and XML cases are constructed in Rust tests to
exercise exact corruption and resource boundaries. No Office content is rendered.

## Milestone 1B presentation fixtures

These additional fixtures exercise presentation metadata and intentionally blank
PDF conversion. Existing Milestone 1A fixture bytes are unchanged.

| File | Purpose |
| --- | --- |
| `one-slide-standard.pptx` | One slide, 4:3, 9,144,000 × 6,858,000 EMUs (720 × 540 points) |
| `three-slides-standard.pptx` | Three slides at the same standard size |
| `three-slides-16x9.pptx` | Three slides, 12,192,000 × 6,858,000 EMUs (960 × 540 points) |
| `reordered-slides.pptx` | Presentation order is slide3, slide1, slide2; IDs are 900, 300, 600 and `.rels` order is different again |
| `missing-slide-size.pptx` | Valid OPC package with no `sldSz`; conversion must fail |
| `malformed-slide-size.pptx` | Valid OPC package with zero slide width; conversion must fail |
| `deep-presentation.pptx` | 65 levels of XML nesting; rejected before recursive tree construction |

Regenerate these fixtures deterministically with:

```sh
cargo run --offline -p o2p-pptx --example generate_fixtures
```

The normalized ordered slide references prove ordering; blank pages of equal
size have no visual slide identity. Tests also check empty PDF content streams,
page counts and MediaBoxes with an independent PDF reader. Presentation-size
errors and nesting errors remain inspectable through the metadata-only package
inventory command, which deliberately does not parse presentation semantics.

## Milestone 1C content fixtures

Generated from original XML by the same Rust regenerator; existing Milestone 1A
and 1B fixture bytes remain unchanged. No proprietary documents are used.

| File | Purpose |
| --- | --- |
| `solid-background.pptx` | Full-slide solid sRGB `304050` background |
| `one-rectangle.pptx` | Black rectangle at (72, 72), size 144 × 72 points |
| `filled-rectangle.pptx` | Blue `2060D0` fill; offset (1,234,567, 765,432) and size (3,456,789, 2,345,678) EMUs, testing fractional point conversion |
| `outlined-rectangle.pptx` | `CCDDEE` fill and `D02030` outline of 31,750 EMUs (2.5 points) |
| `no-fill-rectangle.pptx` | Explicit no fill; outline over an `E0E8F0` background |
| `multiple-rectangles.pptx` | Background and three overlapping rectangles; final outline starts at negative x |
| `reordered-rectangles.pptx` | Blue, red, green pages in slide3/slide1/slide2 reference order |
| `unsupported-style.pptx` | Unresolved scheme fill and dashed outline yield structured warnings and omit paint |
| `invalid-slide-root.pptx` | Well-formed XML with an invalid slide root fails conversion without overwriting output |
| `deep-slide.pptx` | Excessive slide nesting is rejected before recursive XML parsing |

Regenerate with `cargo run --offline -p o2p-pptx --example generate_fixtures`.
Tests compare generated ZIP bytes with committed fixtures, independently parse
rendered PDFs, and verify normalized model values and warnings.
