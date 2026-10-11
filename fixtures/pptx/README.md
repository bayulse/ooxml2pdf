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
