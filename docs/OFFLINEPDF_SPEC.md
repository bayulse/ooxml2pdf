# OfflinePDF

## Private, Fully Offline PDF Toolkit for Android & iOS

**Status:** Draft Product & Engineering Specification\
**Framework:** Flutter\
**Native processing:** Rust\
**Platforms:** Android & iOS\
**Backend:** None\
**Account:** None\
**Network requirement:** None\
**Working application name:** OfflinePDF

---

# 1. Product Vision

OfflinePDF adalah aplikasi mobile untuk mengelola, mengedit, mengonversi, memindai, mengamankan, dan membaca dokumen PDF sepenuhnya di perangkat.

Prinsip produk:

> Your documents never leave your device.

Tidak ada:

- upload
- server
- cloud processing
- login
- account
- analytics SDK
- remote fonts
- ads
- tracking dokumen

Semua pemrosesan dilakukan di Android/iOS secara lokal.

---

# 2. Positioning

OfflinePDF bukan sekadar port Stirling PDF.

Konsepnya:

```text
Stirling-like toolset
        +
mobile-first UX
        +
native Rust processing
        +
zero-server privacy
```

Aplikasi harus terasa seperti aplikasi mobile asli, bukan website yang dibungkus WebView.

---

# 3. Target User

Pengguna yang:

- bekerja dengan dokumen sensitif
- sering menerima PDF melalui chat/email
- perlu merge/split/compress dengan cepat
- bekerja tanpa koneksi stabil
- tidak ingin mengunggah dokumen ke website converter
- membutuhkan scan-to-PDF
- membutuhkan conversion Office → PDF secara lokal

Use case utama:

- pemerintahan
- legal
- corporate
- pendidikan
- personal productivity

---

# 4. Product Principles

1. **Offline first**
2. **Privacy by architecture**
3. **Fast for common actions**
4. **No account**
5. **No surprise upload**
6. **Few taps**
7. **Non-destructive editing**
8. **Native performance**
9. **Clear progress**
10. **Graceful compatibility warnings**

---

# 5. Application Architecture

```text
┌──────────────────────────────────┐
│             Flutter              │
│                                  │
│ UI / Navigation / State          │
│ File Selection / Preview         │
│ Settings / History               │
└───────────────┬──────────────────┘
                │
                │ flutter_rust_bridge
                ▼
┌──────────────────────────────────┐
│        Native Rust Engine        │
│                                  │
│ PDF Operations                   │
│ OOXML → PDF                      │
│ Compression                      │
│ Images                           │
│ Security                         │
│ Text Processing                  │
└───────────────┬──────────────────┘
                │
                ▼
          Local filesystem
```

Platform-native APIs digunakan hanya untuk fungsi seperti:

- file picker
- camera/scanner
- share sheet
- biometric/key storage jika diperlukan
- platform PDF preview bila bermanfaat

---

# 6. Project Structure

```text
offline_pdf/
├── app/
│   ├── lib/
│   │   ├── app/
│   │   ├── core/
│   │   ├── features/
│   │   ├── services/
│   │   ├── widgets/
│   │   └── rust/
│   │
│   ├── android/
│   └── ios/
│
├── rust/
│   ├── Cargo.toml
│   └── crates/
│       ├── mobile-core/
│       ├── pdf-engine/
│       └── flutter-api/
│
└── packages/
    └── ooxml2pdf/
```

Pada fase awal `ooxml2pdf` dapat berada dalam monorepo.

Setelah API stabil, library dipisahkan menjadi repository open-source independen.

---

# 7. Feature Categories

Home screen:

```text
OfflinePDF

Recent files

ORGANIZE
Merge
Split
Reorder
Extract

OPTIMIZE
Compress
Repair

CONVERT
Office to PDF
Image to PDF
PDF to Image

EDIT
Watermark
Page numbers
Crop
Rotate

SCAN & OCR
Scan document
OCR
Extract text

SECURITY
Protect
Unlock
Redact
Sign
```

---

# 8. MVP Feature Set

## Organize

### Merge PDF

User dapat:

- memilih beberapa PDF
- drag reorder
- preview
- merge
- memilih lokasi output

### Split PDF

Mode:

- page range
- every page
- fixed interval

### Extract pages

Contoh:

```text
1,3,5-8
```

### Delete pages

Visual thumbnail selection.

### Rotate

- 90°
- 180°
- 270°

### Reorder

Drag-and-drop thumbnail.

---

# 9. Conversion

## Images → PDF

Input:

- JPEG
- PNG
- HEIC jika platform decoder tersedia
- WebP jika decoder tersedia

Options:

- paper size
- orientation
- margin
- image fit
- image quality

---

## PDF → Images

Options:

- PNG/JPEG
- DPI
- selected pages
- quality

---

# 10. Office → PDF

Powered by:

```text
ooxml2pdf
```

Supported:

```text
DOCX → PDF
XLSX → PDF
PPTX → PDF
```

Pipeline:

```text
Office file
     ↓
Rust OOXML parser
     ↓
Rust layout engine
     ↓
DisplayList
     ↓
PDF writer
     ↓
Local PDF
```

Tidak ada:

```text
server
LibreOffice
WebView
HTML intermediate
cloud API
```

UX:

```text
Converting report.docx

Parsing document      ✓
Resolving fonts       ✓
Laying out pages      71%
Rendering PDF

[ Cancel ]
```

Jika ada unsupported feature:

```text
PDF created successfully

2 elements may look different from the original:
• SmartArt on slide 8
• Chart on slide 14
```

---

# 11. Compress PDF

Presets:

```text
Light
Balanced
Strong
Custom
```

Jangan memberikan klaim palsu bahwa setiap PDF akan mengecil.

Result screen:

```text
Before   14.8 MB
After     5.2 MB

Saved    64.9%
```

---

# 12. Scan to PDF

Flow:

```text
Camera
 ↓
document boundary
 ↓
perspective correction
 ↓
crop
 ↓
filter
 ↓
page list
 ↓
PDF
```

Modes:

- color
- grayscale
- black & white

User dapat:

- reorder
- rotate
- crop ulang
- delete
- add pages

---

# 13. OCR

OCR sepenuhnya lokal.

Target awal:

- English
- Indonesian

Model tambahan dapat diunduh pada versi mendatang hanya jika user secara eksplisit memilihnya.

Untuk prinsip strictly offline pada base application, model inti dibundel.

Output:

- searchable PDF
- extracted text
- copy text

---

# 14. Security Tools

## Protect PDF

Options:

- password
- print permission
- copy permission
- edit permission

## Unlock

Hanya setelah user memberikan password yang benar.

## Redact

Redaction harus benar-benar menghapus atau mengganti konten, bukan sekadar menaruh rectangle visual.

UI:

```text
Select area
 ↓
Mark for redaction
 ↓
Review
 ↓
Apply permanently
```

## Digital Signature

Later phase.

Target:

- PKCS#12/PFX
- certificate selection
- visible signature
- invisible digital signature

---

# 15. PDF Viewer

Viewer harus mendukung:

- pinch zoom
- vertical scrolling
- page jump
- thumbnail navigation
- text search
- dark surrounding UI
- rotate
- share
- open in another application

Viewer tidak perlu menjadi full Adobe Acrobat clone.

---

# 16. Home UX

Contoh:

```text
┌─────────────────────────────┐
│ OfflinePDF                  │
│                             │
│ Your files stay on device   │
│                             │
│  Merge       Split          │
│                             │
│  Compress    Convert        │
│                             │
│  Scan        OCR            │
│                             │
│  Edit        Protect        │
│                             │
│ Recent                      │
│ ─────────────────────────   │
│ report.pdf                  │
│ contract.pdf                │
└─────────────────────────────┘
```

Tidak perlu dashboard rumit.

Aksi utama harus dapat ditemukan dalam satu layar.

---

# 17. Tool Workflow Pattern

Semua tool mengikuti pola konsisten:

```text
Select
  ↓
Configure
  ↓
Preview
  ↓
Process
  ↓
Result
```

Contoh merge:

```text
Select PDFs
   ↓
Arrange
   ↓
Merge
   ↓
Preview
   ↓
Save / Share
```

---

# 18. Files

Aplikasi sebaiknya tidak membangun custom cloud filesystem.

Gunakan:

### Android

Storage Access Framework.

### iOS

Document Picker / Files.

Dokumen diproses ke temporary working directory.

Result kemudian:

- Save
- Share
- Open in...
- Replace original, hanya jika user meminta

---

# 19. Temporary Files

Setiap operation memiliki workspace:

```text
tmp/jobs/<uuid>/
```

Contoh:

```text
tmp/jobs/abc123/
├── input-1.pdf
├── input-2.pdf
├── thumbnails/
└── output.pdf
```

Setelah:

```text
success
cancel
failure
```

workspace harus dibersihkan.

Recovery cleanup dilakukan saat application launch.

---

# 20. Privacy Architecture

Aplikasi tidak boleh mempunyai dependency yang diam-diam melakukan network access.

Tidak menggunakan:

- Firebase Analytics
- Crashlytics
- Sentry cloud
- advertising SDK
- remote feature flags
- remote fonts
- user tracking SDK

---

# 21. Android Internet Permission

Release Android idealnya tidak mendeklarasikan:

```xml
android.permission.INTERNET
```

Dengan demikian privacy claim dapat diverifikasi langsung pada permission aplikasi.

Jika suatu hari fitur network ditambahkan, harus menjadi keputusan produk eksplisit dan bukan dependency side effect.

---

# 22. iOS Networking

Tidak ada request HTTP yang dilakukan oleh application code.

App Privacy disclosure harus konsisten dengan implementasi.

---

# 23. Local History

Recent files sebaiknya hanya menyimpan:

```text
display name
last used
operation
local document reference/bookmark
```

Bukan:

- document content
- extracted OCR text
- user behavior telemetry

User dapat:

```text
Settings
→ Clear recent files
```

---

# 24. Flutter ↔ Rust

Gunakan:

```text
flutter_rust_bridge
```

Flutter memanggil API tinggi, bukan primitive internal.

Contoh:

```dart
final job = await rustApi.mergePdf(
  inputs: files,
  output: outputPath,
);
```

dan:

```dart
final job = await rustApi.officeToPdf(
  input: path,
  output: target,
);
```

---

# 25. Long Running Jobs

Operasi besar harus mendukung:

- progress
- cancellation
- error reporting
- warnings

Model:

```dart
sealed class ProcessingEvent {}

class Started extends ProcessingEvent {}
class Progress extends ProcessingEvent {}
class Warning extends ProcessingEvent {}
class Completed extends ProcessingEvent {}
class Failed extends ProcessingEvent {}
```

UI tidak boleh freeze.

---

# 26. Error UX

Jangan menampilkan:

```text
Error: ZipError(InvalidArchive("..."))
```

kepada user.

Mapping:

```text
Technical error
      ↓
Domain error
      ↓
User message
```

Contoh:

```text
This Word document appears to be damaged
or uses features that are not supported yet.
```

Detail teknis tersedia melalui:

```text
Show technical details
```

untuk debugging.

---

# 27. State Management

Tidak perlu architecture yang terlalu kompleks.

Proposal:

```text
Riverpod
```

atau equivalent yang sederhana dan testable.

Pemisahan:

```text
presentation
domain
infrastructure
```

Tetapi hindari excessive clean-architecture boilerplate.

---

# 28. UI Design

Gunakan:

- Material 3 foundation
- adaptive Cupertino behavior
- responsive phone/tablet layout

Visual:

```text
clean
quiet
document-focused
minimal
```

Bukan:

```text
dashboard-heavy
enterprise portal
50 cards dalam satu layar
```

---

# 29. Tablet

Tablet mendapat keuntungan dari split view:

```text
┌─────────────┬──────────────────────┐
│ thumbnails  │                      │
│             │      PDF page        │
│             │                      │
│             │                      │
└─────────────┴──────────────────────┘
```

Phone tetap single-column.

---

# 30. Accessibility

Minimum:

- semantic labels
- dynamic text sizing
- screen reader labels
- large tap targets
- high contrast
- keyboard support di tablet bila memungkinkan

---

# 31. Performance Rules

Jangan membaca seluruh file ke Dart memory jika tidak diperlukan.

Prefer:

```text
Flutter path
    ↓
Rust reads file directly
```

bukan:

```text
Flutter loads 300 MB
    ↓
Uint8List
    ↓
copies over FFI
```

File path / native handle harus menjadi default API untuk file besar.

---

# 32. Thumbnail Strategy

Thumbnail harus lazy.

```text
visible pages
   ↓
render thumbnails

offscreen pages
   ↓
render on demand
```

Cache dibatasi berdasarkan:

- item count
- memory
- disk size

---

# 33. Application Size

Feature modularity harus dipertahankan.

Rust:

```toml
features = [
  "pdf-basic",
  "docx",
  "xlsx",
  "pptx",
  "ocr"
]
```

Jika OCR membuat binary terlalu besar, ia bisa menjadi optional build feature atau language packs dikelola terpisah.

---

# 34. MVP Release

## V0.1 — Internal Prototype

Tools:

- PDF viewer
- merge
- split
- rotate
- reorder
- images → PDF
- PDF → images

Plus:

```text
PPTX → PDF experimental
```

---

## V0.2

Tambah:

- compress
- password protect
- unlock
- watermark
- page numbers

Office:

```text
PPTX → PDF
XLSX → PDF experimental
```

---

## V0.3

Tambah:

- scan
- crop
- delete/extract pages

Office:

```text
DOCX → PDF experimental
```

---

## V0.4

Tambah:

- OCR
- searchable PDF
- extract text

Office conversion masuk beta.

---

## V0.5

Tambah:

- redaction
- metadata
- flatten
- advanced compression

---

# 35. V1.0 Requirements

Tidak perlu memiliki seluruh fitur Stirling.

V1 cukup jika memiliki tool yang reliable:

```text
Merge
Split
Extract
Delete
Reorder
Rotate

Compress

Image → PDF
PDF → Image
DOCX → PDF
XLSX → PDF
PPTX → PDF

Scan
OCR

Watermark
Page number

Protect
Unlock
Redact
```

Lebih baik 18 tool yang sangat bagus daripada 60 tool setengah matang.

---

# 36. Testing

## Flutter

- widget test
- state test
- navigation test
- golden UI test

## Rust

- unit
- integration
- fuzz
- rendering regression

## Device Matrix

Minimum:

```text
Android
- entry/mid-range arm64
- flagship arm64
- emulator x86_64

iOS
- older supported iPhone
- current iPhone
- iPad
```

Test khusus:

- low memory
- background/foreground
- cancellation
- huge PDF
- password-protected file
- malformed input

---

# 37. CI/CD

Pull request:

```text
Flutter analyze
Flutter test
cargo fmt
cargo clippy
cargo test
cargo deny
build Android native library
build iOS native library
```

Release:

```text
tag
 ↓
build
 ↓
sign
 ↓
integration test
 ↓
Play Store / App Store
```

---

# 38. Security

Dokumen adalah untrusted input.

App harus tahan terhadap:

- malformed PDF
- malicious OOXML
- ZIP bombs
- oversized images
- corrupted fonts
- path traversal
- excessive memory allocation

Semua file parsing utama sebisa mungkin berada di memory-safe Rust.

---

# 39. Logging

Production logging:

```text
allowed:
operation type
duration
generic error

not allowed:
document content
OCR text
password
signature key
full sensitive path
```

Log tetap lokal.

---

# 40. Monetization, Jika Diperlukan

Architecture tidak bergantung pada SaaS.

Model memungkinkan:

```text
Free
+
one-time Pro purchase
```

lebih natural daripada subscription untuk aplikasi offline.

Tetapi monetization tidak diperlukan pada fase open-source/MVP.

Core Rust `ooxml2pdf` tetap open-source dan terpisah dari keputusan monetisasi aplikasi.

---

# 41. Relationship dengan ooxml2pdf

```text
┌────────────────────────────┐
│       ooxml2pdf            │
│ Open-source Rust library   │
│                            │
│ DOCX ─┐                    │
│ XLSX ─┼──→ PDF             │
│ PPTX ─┘                    │
└────────────┬───────────────┘
             │
             ▼
┌────────────────────────────┐
│        OfflinePDF          │
│ Flutter Android / iOS      │
│                            │
│ viewer                     │
│ PDF tools                  │
│ scanner                    │
│ OCR                        │
│ security                   │
│ Office conversion          │
└────────────────────────────┘
```

Library harus tetap berguna tanpa aplikasi.

Aplikasi juga tidak boleh mengambil alih tanggung jawab renderer Office.

---

# 42. Recommended Development Order

Jangan mulai dari UI lengkap.

Urutan:

```text
1. Rust workspace
2. PDF primitive renderer
3. PPTX → PDF proof-of-concept
4. Flutter ↔ Rust bridge
5. tampilkan hasil PDF di Flutter
6. PDF merge/split/basic tools
7. XLSX renderer
8. DOCX flow renderer
9. scanner
10. OCR
11. polish UX
```

Critical path:

```text
PPTX file
   ↓
Rust
   ↓
PDF
   ↓
Flutter preview
```

Begitu pipeline ini berhasil di Android dan iOS, fondasi produk telah terbukti.

---

# 43. Initial Proof of Concept

POC pertama hanya perlu satu layar:

```text
┌──────────────────────────┐
│ OOXML2PDF Test           │
│                          │
│ [ Choose Office File ]   │
│                          │
│ presentation.pptx        │
│                          │
│ [ Convert to PDF ]       │
│                          │
│ Parsing       ✓          │
│ Rendering     83%        │
│                          │
└──────────────────────────┘
```

Setelah selesai:

```text
┌──────────────────────────┐
│ ✓ Converted              │
│                          │
│ presentation.pdf         │
│ 12 pages                 │
│                          │
│ [ Preview ]              │
│ [ Share ]                │
└──────────────────────────┘
```

Jangan membangun home screen final sebelum pipeline tersebut bekerja.

---

# 44. Product Definition

OfflinePDF harus dapat menjawab satu pertanyaan dengan sangat jelas:

> Apakah dokumen saya pernah meninggalkan perangkat?

Jawaban yang benar harus selalu:

> Tidak.

Bukan karena privacy policy mengatakan demikian, tetapi karena arsitektur aplikasi memang tidak mempunyai mekanisme untuk mengirim dokumen keluar.

---

# 45. Long-Term Direction

Setelah V1:

```text
PDF forms
digital signatures
PDF/A
advanced OCR
batch processing
automation/shortcuts
desktop version
share-extension processing
folder processing
```

Tetapi semuanya tetap tunduk pada prinsip:

```text
Offline
Private
Native
Reliable
Simple
```

OfflinePDF bukan sekadar kumpulan PDF tools.

Produk ini adalah **private document processing toolkit yang seluruh processing stack-nya dimiliki dan dijalankan secara lokal**.
