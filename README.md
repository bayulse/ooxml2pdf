# ooxml2pdf

A native Rust foundation for an offline OOXML-to-PDF engine.

Milestone 0 renders a format-independent DisplayList containing text, filled
rectangles and lines into a PDF. Office format parsing is not implemented yet.

Generate the deterministic demo:

```sh
cargo run -p o2p-cli -- demo target/demo.pdf
```

The demo contains one page, a rectangle, two text runs and a line. The command
overwrites an existing output file; its parent directory must exist.

The workspace contains `o2p-core` (dependency-free drawing model), `o2p-pdf`
(PDF bytes), and `o2p-cli` (`o2p` executable). Rendering is pure Rust and needs
no network access, system fonts, browser, LibreOffice or external executable.
The initial backend supports printable ASCII in standard Helvetica.

```sh
cargo fmt
cargo clippy --all-targets --all-features
cargo test --workspace
```

See [the architecture decisions](docs/ARCHITECTURE.md) and
[the product specification](docs/PRODUCT_SPEC.md).
