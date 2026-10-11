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
