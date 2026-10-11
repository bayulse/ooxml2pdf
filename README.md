# ooxml2pdf

A native Rust foundation for an offline OOXML-to-PDF engine.

Milestone 0 renders a format-independent DisplayList containing text, filled
rectangles and lines into a PDF. Milestone 1A opens and inspects PPTX packages;
slide content parsing and Office-to-PDF conversion are not implemented yet.

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
(PDF bytes), `o2p-ooxml` (OOXML / OPC packages), and `o2p-cli` (`o2p` executable).
Processing is pure Rust and needs
no network access, system fonts, browser, LibreOffice or external executable.
The initial backend supports printable ASCII in standard Helvetica.

```sh
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

See [the architecture decisions](docs/ARCHITECTURE.md) and
[the product specification](docs/PRODUCT_SPEC.md).
