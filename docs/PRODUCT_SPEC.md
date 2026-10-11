# ooxml2pdf

## Open-Source Native OOXML → PDF Rendering Engine in Rust

**Status:** Draft Design Specification\
**Language:** Rust\
**License proposal:** MIT OR Apache-2.0\
**Primary targets:** Android, iOS, Linux, macOS, Windows\
**Secondary target:** WASM, jika dependency memungkinkan\
**Repository working name:** `ooxml2pdf`

---

# 1. Ringkasan

`ooxml2pdf` adalah library open-source berbasis Rust untuk mengubah dokumen Microsoft Office Open XML menjadi PDF tanpa Microsoft Office, LibreOffice, Chromium, WebView, Java, Node.js, atau layanan cloud.

Format awal:

- `.docx` → PDF
- `.xlsx` → PDF
- `.pptx` → PDF

Target utama bukan membuat Office suite baru, melainkan membuat **renderer OOXML → PDF yang ringan, deterministic, cross-platform, offline, dan mudah diintegrasikan ke aplikasi native**.

Library harus dapat digunakan dari:

- Rust
- Flutter melalui FFI
- CLI
- Android
- iOS
- desktop

Target fidelity adalah dokumen kantor umum, bukan kompatibilitas pixel-perfect terhadap seluruh perilaku Microsoft Office.

---

# 2. Filosofi

## 2.1 Prinsip utama

1. Pure native processing.
2. Tidak membutuhkan koneksi internet.
3. Tidak menjalankan external process.
4. Tidak menggunakan LibreOffice.
5. Tidak menggunakan Chromium/WebView.
6. Tidak menjalankan JavaScript.
7. Memory usage harus dapat diprediksi.
8. Input yang rusak atau malicious tidak boleh menyebabkan process crash.
9. Core engine tidak boleh bergantung pada Flutter.
10. Semua renderer menghasilkan intermediate representation yang sama.

---

# 3. Bukan Tujuan Project

`ooxml2pdf` tidak bertujuan menjadi:

- Word editor
- Excel editor
- PowerPoint editor
- Office macro runtime
- VBA interpreter
- collaboration suite
- Office server
- full Office-to-Office converter
- pixel-perfect Microsoft Office clone

V1 hanya menjawab:

> Diberikan file DOCX/XLSX/PPTX, hasilkan representasi PDF yang semirip mungkin dengan dokumen sumber menggunakan engine native Rust.

---

# 4. Arsitektur Tingkat Tinggi

```text
                         Input

             DOCX         XLSX         PPTX
               │            │            │
               └────────────┼────────────┘
                            │
                            ▼
                  OOXML Package Layer
                  ZIP / OPC / Relations
                            │
                 ┌──────────┼──────────┐
                 ▼          ▼          ▼
              WML Parser  SML Parser  PML Parser
                 │          │          │
                 └──────────┼──────────┘
                            ▼
                Normalized Document Model
                            │
                 ┌──────────┼──────────┐
                 ▼          ▼          ▼
               Word       Sheet       Slide
              Layout      Layout      Layout
                 │          │          │
                 └──────────┼──────────┘
                            ▼
                     Display List
                            │
                    ┌───────┼───────┐
                    ▼       ▼       ▼
                   Text   Vector   Image
                            │
                            ▼
                       PDF Backend
                            │
                            ▼
                         PDF file
```

---

# 5. Workspace Rust

Struktur awal:

```text
ooxml2pdf/
├── Cargo.toml
├── LICENSE-MIT
├── LICENSE-APACHE
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── CHANGELOG.md
├── docs/
│   ├── architecture.md
│   ├── fidelity.md
│   ├── security.md
│   ├── fonts.md
│   └── format-support.md
│
├── crates/
│   ├── o2p-core/
│   ├── o2p-ooxml/
│   ├── o2p-docx/
│   ├── o2p-xlsx/
│   ├── o2p-pptx/
│   ├── o2p-layout/
│   ├── o2p-fonts/
│   ├── o2p-pdf/
│   ├── o2p-cli/
│   └── o2p-flutter/
│
├── fixtures/
│   ├── docx/
│   ├── xlsx/
│   ├── pptx/
│   └── fonts/
│
└── tests/
    ├── rendering/
    ├── fuzz/
    └── regression/
```

---

# 6. Crate Responsibility

## `o2p-core`

Berisi tipe fundamental:

- unit
- point
- rectangle
- transform
- color
- page size
- resources
- errors
- warnings
- configuration

Tidak mengetahui DOCX/XLSX/PPTX.

---

## `o2p-ooxml`

Menangani:

- ZIP/OPC package
- relationship
- content type
- resource lookup
- media extraction
- XML access
- theme
- shared DrawingML resources

Parser konkret berada di balik abstraction.

```rust
pub trait PackageReader {
    fn part(&self, path: &str) -> Result<&[u8]>;
    fn relationship(&self, id: &str) -> Option<Relationship>;
}
```

Tujuannya agar dependency parser OOXML dapat diganti.

---

# 7. Parser Backend

Kita tidak perlu membuat seluruh parser XML OOXML dari nol.

Backend awal dapat memanfaatkan crate open-source yang sudah tersedia.

Tetapi core tidak boleh expose tipe dependency tersebut.

Contoh:

```rust
pub trait DocxParser {
    fn parse(&self, input: &[u8]) -> Result<WordDocument>;
}

pub trait XlsxParser {
    fn parse(&self, input: &[u8]) -> Result<Workbook>;
}

pub trait PptxParser {
    fn parse(&self, input: &[u8]) -> Result<Presentation>;
}
```

Sehingga:

```text
OOXML crate A
      │
      ├── adapter
      ▼
o2p normalized model

OOXML crate B
      │
      ├── adapter
      ▼
o2p normalized model
```

Layout engine tetap sama.

---

# 8. Common Rendering Model

Semua format akhirnya menghasilkan display list yang independen dari Office.

```rust
pub struct Page {
    pub size: Size,
    pub operations: Vec<DrawOp>,
}

pub enum DrawOp {
    Text(TextOp),
    Image(ImageOp),
    Path(PathOp),
    Rect(RectOp),
    Line(LineOp),
    Clip(ClipOp),
    Transform(TransformOp),
}
```

Contoh:

```rust
DrawOp::Text(TextOp {
    text: "Kementerian PANRB".into(),
    x: 72.0,
    y: 100.0,
    font_size: 14.0,
    font_weight: FontWeight::Bold,
    ..
})
```

PDF backend tidak perlu memahami konsep:

- Word paragraph
- Excel cell
- PowerPoint shape

Ia hanya memahami primitive drawing.

---

# 9. Typography

Text rendering adalah komponen kritis.

Engine membutuhkan:

- shaping
- kerning
- ligature
- Unicode
- font fallback
- RTL
- line breaking
- font metrics
- bold/italic
- underline
- strike-through
- letter spacing
- baseline adjustment
- superscript/subscript

Text stack awal:

```text
OOXML run
   ↓
resolved font/style
   ↓
font resolver
   ↓
text shaper
   ↓
glyph positions
   ↓
DisplayList
```

`cosmic-text` dapat dijadikan kandidat awal untuk shaping dan layout.

Kita tetap membuat abstraction:

```rust
pub trait TextShaper {
    fn shape(
        &self,
        text: &str,
        style: &TextStyle,
        constraints: TextConstraints,
    ) -> Result<ShapedText>;
}
```

---

# 10. Font Strategy

Font merupakan salah satu sumber utama perbedaan layout.

Urutan resolusi:

```text
Embedded font
     ↓
Bundled compatible font
     ↓
System font
     ↓
Configured fallback
     ↓
Noto fallback
```

Contoh mapping opsional:

```text
Calibri → Carlito
Arial → Liberation Sans
Times New Roman → Liberation Serif
Cambria → Caladea
```

Library tidak wajib membundel font.

Consumer dapat menyediakan:

```rust
RenderOptions {
    font_paths: vec![...],
    font_substitutions: ...,
}
```

---

# 11. DOCX Renderer

DOCX adalah renderer paling kompleks karena menggunakan flow layout.

Model awal:

```text
Document
└── Section
    ├── Paragraph
    │   └── Run
    ├── Table
    ├── Image
    ├── PageBreak
    └── SectionBreak
```

Layout pipeline:

```text
resolve styles
      ↓
resolve paragraph properties
      ↓
shape runs
      ↓
line breaking
      ↓
block measurement
      ↓
page fitting
      ↓
pagination
      ↓
DisplayList
```

## V0

- paragraph
- run
- font
- bold
- italic
- underline
- text color
- font size
- simple alignment
- images
- explicit page break

## V1

Tambahkan:

- tables
- margins
- headers
- footers
- numbered lists
- bullet lists
- paragraph spacing
- indentation
- line spacing
- sections
- page orientation
- hyperlinks
- basic footnotes

## V2

Tambahkan:

- floating images
- text boxes
- columns
- advanced table pagination
- complex field codes
- equations
- comments
- tracked-change rendering

---

# 12. XLSX Renderer

Spreadsheet renderer berdasarkan print layout.

Model:

```text
Workbook
└── Worksheet
    ├── Row
    ├── Column
    ├── Cell
    ├── MergedRange
    ├── Drawing
    └── PrintSettings
```

V1 harus menangani:

- values
- cached formula results
- number formatting
- dates
- fonts
- colors
- fills
- borders
- alignment
- wrap text
- column width
- row height
- merged cells
- print area
- orientation
- margins
- scaling
- page breaks

Tidak perlu menghitung seluruh formula Excel pada awal project.

Prioritas:

```text
cached formula result > formula evaluation
```

## Later

- charts
- conditional formatting
- print titles
- advanced drawings
- pivot rendering

---

# 13. PPTX Renderer

PPTX diprioritaskan untuk prototype karena layout mayoritas sudah absolute-positioned.

Pipeline:

```text
slide
 ↓
slide master
 ↓
layout
 ↓
theme
 ↓
shape tree
 ↓
coordinate conversion
 ↓
DisplayList
```

V0:

- text box
- image
- rectangle
- ellipse
- lines
- fills
- borders
- basic AutoShapes

V1:

- groups
- tables
- master slide
- layouts
- theme colors
- gradients
- SVG
- rotation
- transparency

Later:

- charts
- SmartArt
- WordArt
- complex effects

Animations tidak relevan terhadap PDF dan tidak perlu diimplementasikan.

---

# 14. PDF Backend

PDF output berada di crate terpisah.

```rust
pub trait DocumentRenderer {
    fn render(
        &self,
        pages: &[Page],
        output: &mut dyn Write,
        options: &PdfOptions,
    ) -> Result<RenderReport>;
}
```

Backend awal dapat menggunakan `printpdf`.

Namun display list kita tidak boleh bergantung pada tipe `printpdf`.

Dengan demikian backend dapat diganti di masa depan.

---

# 15. Public Rust API

Target API sederhana:

```rust
use ooxml2pdf::{convert, ConvertOptions};

convert(
    "document.docx",
    "document.pdf",
    ConvertOptions::default(),
)?;
```

API yang lebih eksplisit:

```rust
let report = ooxml2pdf::convert_file(
    "presentation.pptx",
    "presentation.pdf",
    RenderOptions {
        quality: Quality::High,
        ..
    },
)?;
```

Return value:

```rust
pub struct RenderReport {
    pub input_format: InputFormat,
    pub page_count: usize,
    pub warnings: Vec<RenderWarning>,
    pub unsupported_features: Vec<UnsupportedFeature>,
}
```

---

# 16. Warning, Bukan Silent Failure

Jika ditemukan unsupported element:

```text
SmartArt unsupported
```

jangan:

```text
drop silently
```

Tetapi:

```rust
RenderWarning {
    kind: UnsupportedFeature::SmartArt,
    location: "ppt/slides/slide3.xml",
}
```

Consumer dapat menampilkan:

> Converted successfully with 2 compatibility warnings.

---

# 17. CLI

Binary:

```text
o2p
```

Contoh:

```bash
o2p document.docx
```

menghasilkan:

```text
document.pdf
```

Opsi:

```bash
o2p input.docx -o output.pdf
o2p report.xlsx --quality high
o2p deck.pptx --font-dir ./fonts
o2p document.docx --json-report report.json
```

Inspection:

```bash
o2p inspect document.docx
```

Output:

```text
Format: DOCX
Pages estimated: 8
Fonts:
- Calibri
- Arial

Unsupported:
- SmartArt: 1
```

---

# 18. Flutter API

Bridge tidak berada di core crate.

```text
Flutter
  ↓
flutter_rust_bridge
  ↓
o2p-flutter
  ↓
ooxml2pdf core
```

API Dart:

```dart
final result = await convertOfficeToPdf(
  inputPath: input,
  outputPath: output,
  options: const OfficeConvertOptions(),
);
```

Progress:

```dart
convertOfficeToPdf(...).listen((event) {
  switch (event) {
    case ParsingProgress():
    case LayoutProgress():
    case RenderingProgress():
  }
});
```

---

# 19. Feature Flags

```toml
[features]
default = ["docx", "xlsx", "pptx"]

docx = []
xlsx = []
pptx = []

svg = []
charts = []
math = []
embedded-fonts = []
```

Consumer dapat membuat build yang lebih kecil:

```toml
ooxml2pdf = {
  version = "...",
  default-features = false,
  features = ["docx", "pptx"]
}
```

---

# 20. Performance Target

Target awal untuk smartphone kelas menengah:

### Small document

```text
< 20 pages
< 10 MB
```

Conversion harus terasa interaktif.

### Medium document

```text
20–100 pages
10–50 MB
```

Tidak boleh menyebabkan UI freeze.

### Large document

```text
100+ pages
50+ MB
```

Engine harus mengutamakan bounded memory dan streaming jika memungkinkan.

Aturan desain:

- jangan menyimpan semua decoded image jika tidak diperlukan
- cache font
- cache glyph
- release page resources setelah selesai
- limits untuk decompression
- limits untuk image dimensions
- limits untuk XML nesting

---

# 21. Security

Input dianggap tidak dipercaya.

Threats:

- ZIP bomb
- decompression bomb
- XML entity abuse
- excessive relationships
- malformed image
- huge image dimensions
- recursive structure
- integer overflow
- resource exhaustion
- corrupted fonts

Default limits harus tersedia:

```rust
SecurityLimits {
    max_input_size,
    max_uncompressed_size,
    max_zip_entries,
    max_xml_depth,
    max_image_pixels,
    max_pages,
    max_relationships,
}
```

Tidak boleh ada network resource loading.

External links tidak pernah di-fetch.

---

# 22. Privacy

Engine:

- tidak melakukan network call
- tidak mempunyai analytics
- tidak mempunyai telemetry
- tidak menyimpan dokumen
- tidak mengirim metadata
- tidak memerlukan account

Privacy menjadi architectural invariant.

---

# 23. Testing Strategy

## Unit tests

Untuk:

- unit conversion
- theme resolution
- style inheritance
- XML mapping
- table calculation
- line breaking
- pagination

## Golden rendering tests

Fixture:

```text
input.docx
expected/page-001.png
expected/page-002.png
```

Pipeline CI:

```text
OOXML
 ↓
PDF
 ↓
rasterize
 ↓
pixel / perceptual comparison
```

Toleransi diberikan untuk antialiasing.

## Cross-reference test

Dokumen fixture dirender juga menggunakan:

- Microsoft Office
- LibreOffice

Output dijadikan reference image.

## Regression

Setiap bug rendering harus menambah fixture permanen.

---

# 24. Fuzzing

Gunakan `cargo-fuzz`.

Target:

- OOXML package parser
- relationship parser
- DrawingML parser
- malformed XML
- corrupted ZIP
- malformed image
- layout input

Goal:

```text
arbitrary file → error/result
```

bukan:

```text
arbitrary file → panic/crash/OOM
```

---

# 25. Fidelity Metrics

Jangan memakai klaim abstrak seperti:

> 95% compatible.

Buat corpus nyata.

Kategori:

```text
DOCX
- letters
- reports
- tables
- government documents
- academic documents

XLSX
- tabular report
- finance
- formatted tables
- print layouts

PPTX
- corporate slides
- government slides
- image-heavy
- charts
```

Score:

```text
Perfect
Minor difference
Usable
Major difference
Failed
```

---

# 26. Roadmap

## Milestone 0 — Foundation

- Cargo workspace
- core geometry
- display list
- font subsystem prototype
- PDF output
- CI
- fixtures

## Milestone 1 — PPTX Prototype

Target:

```text
PPTX → PDF
```

Support:

- slide size
- text
- image
- rectangle
- ellipse
- lines
- basic styles

Tujuan utama:

> membuktikan full pipeline OOXML → native Rust → PDF.

## Milestone 2 — XLSX Prototype

Support:

- cells
- merged cells
- fonts
- fill
- border
- alignment
- row/column sizes
- print settings

## Milestone 3 — DOCX Prototype

Support:

- paragraph
- runs
- wrapping
- images
- page breaks
- simple tables

## Milestone 4 — Flutter Integration

- Android arm64
- Android x86_64 emulator
- iOS arm64
- progress reporting
- cancellation
- error model

## Milestone 5 — Compatibility

- headers/footer
- sections
- slide masters
- spreadsheet print areas
- advanced tables
- font substitution

## Milestone 6 — Stable 1.0

Requirements:

- documented public API
- SemVer
- fuzzing
- security policy
- 100+ fixture corpus
- Android/iOS production validation

---

# 27. Versioning

Pre-stable:

```text
0.1 → architecture
0.2 → PPTX
0.3 → XLSX
0.4 → DOCX
0.5 → Flutter
...
```

`1.0` hanya ketika public API dianggap stabil.

---

# 28. Open-Source Governance

Repository harus menyediakan:

```text
README.md
CONTRIBUTING.md
CODE_OF_CONDUCT.md
SECURITY.md
CHANGELOG.md
LICENSE-MIT
LICENSE-APACHE
```

Issue labels:

```text
format:docx
format:xlsx
format:pptx
area:layout
area:fonts
area:pdf
area:security
area:performance
good-first-issue
fidelity
regression
```

---

# 29. Licensing

Own code:

```text
MIT OR Apache-2.0
```

Setiap dependency baru wajib diperiksa lisensinya.

CI dapat menggunakan:

```text
cargo-deny
```

untuk:

- license auditing
- security advisories
- dependency policy

Font tidak boleh dibundel tanpa memastikan redistributable license.

---

# 30. Definition of Success

Project dianggap berhasil jika:

```text
DOCX ─┐
XLSX ─┼──→ native Rust → PDF
PPTX ─┘
```

berjalan:

- tanpa server
- tanpa network
- tanpa LibreOffice
- tanpa browser
- tanpa JavaScript
- di Android
- di iOS

dan menghasilkan output yang baik untuk mayoritas dokumen kantor umum.

---

# 31. Prinsip Scope

Jika harus memilih antara:

```text
1000 fitur setengah jadi
```

dan:

```text
100 fitur sangat reliable
```

pilih yang kedua.

Urutan prioritas:

```text
correctness
→ security
→ fidelity
→ stability
→ performance
→ feature breadth
```

`ooxml2pdf` harus menjadi **renderer kecil yang sangat dapat dipercaya**, bukan Office suite raksasa.
