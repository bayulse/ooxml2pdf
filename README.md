# ooxml2pdf

A native Rust foundation for an offline OOXML-to-PDF engine.

Milestone 0 renders a format-independent DisplayList containing text, filled
rectangles and lines into a PDF. Milestone 1A opens and inspects PPTX packages;
Milestone 1B converts PPTX metadata into blank PDF pages with the presentation's
real dimensions and slide order. Milestone 1C renders explicit solid slide
backgrounds and rectangles with solid fills or basic outlines. Milestone 1D adds
ellipses and straight lines with explicit solid strokes. Milestone 1E adds
embedded PNG/JPEG pictures in slide paint order.

Generate the deterministic demo:

```sh
cargo run -p o2p-cli -- demo target/demo.pdf
```

The demo contains one page, a rectangle, two text runs and a line. The command
overwrites an existing output file; its parent directory must exist.

Inspect a PPTX package without rendering its slide contents:

```sh
cargo run -p o2p-cli -- inspect fixtures/pptx/one-image.pptx
```

The command lists slide and media part names and counts. The package library
validates metadata, relationships, safe paths, ZIP integrity and configurable
resource limits; external relationships are preserved and never fetched.

The workspace contains `o2p-core` (dependency-free drawing model), `o2p-pdf`
(PDF bytes), `o2p-ooxml` (OOXML / OPC packages), `o2p-pptx` (presentation metadata
and normalized slide content), and `o2p-cli` (`o2p` executable).
Processing is pure Rust and needs
no network access, system fonts, browser, LibreOffice or external executable.
The initial backend supports printable ASCII in standard Helvetica.

Convert a PPTX into PDF pages:

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/three-slides-16x9.pptx -o target/three-slides.pdf
```

Each referenced slide produces one page in presentation order; this example
creates three 960 × 540 point pages. Slide filenames do not determine order.
Explicit sRGB solid backgrounds, axis-aligned rectangles and ellipses support
solid/no fill and simple solid outlines. Straight lines support position, size,
flipped endpoints, color and width. Embedded PNG/JPEG pictures fill their explicit
transform rectangles; crop/tile, picture flips/rotation and effects are warned
about and omitted. Text, themes, inherited styles and other
features remain unsupported and produce structured warnings on stderr.
Missing/invalid slide sizes, malformed referenced slide XML, or slide
relationships fail before writing output. Existing output files are overwritten
after successful parsing/rendering; the parent directory must exist.

Render the synthetic filled-rectangle example:

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/filled-rectangle.pptx -o target/filled-rectangle.pdf
```

Render all three supported shapes:

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/mixed-shapes.pptx -o target/mixed-shapes.pdf
```

Render embedded raster pictures alongside vector shapes:

```sh
cargo run -p o2p-cli -- convert fixtures/pptx/image-with-shapes.pptx -o target/image-with-shapes.pdf
```

Malformed supported media fails before writing output. The backend bounds image
decoding to 16,777,216 pixels, 16,384 pixels per dimension and 128 MiB decoder
allocation. Resources share encoded bytes; decoded pixels are released per image.

```sh
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

See [the architecture decisions](docs/ARCHITECTURE.md) and
[the product specification](docs/PRODUCT_SPEC.md).
