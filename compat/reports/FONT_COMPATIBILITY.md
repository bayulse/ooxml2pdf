# Font Compatibility Fix — scalable outline fonts with embedded bitmap strikes

Completed 2026-10-13 against committed baseline `585dcaf` (Fidelity 2C).
Existing untracked local font assets/scripts were left untouched. No dependency,
layout heuristic, bitmap renderer, directory limit, reference or threshold changes.

## Classification defect and generic correction

`FontBook::register_file` calls `register_bytes`, `FontFace::from_bytes`, then
`FontFace::from_resource`. The baseline checked a table-record denylist before
validating required TrueType outlines. Any EBDT/EBLC record caused
`UnsupportedTechnology("bitmap font")`, even alongside usable glyf/loca outlines.
Local Microsoft PowerPoint for Mac Calibri.ttf reproduced that exact error.

The private classifier now recognizes glyf + loca as a supported outline
candidate before checking auxiliary bitmap technologies. Registration still
requires all existing bounds-checked required-table, parsed-outline/metrics,
embedding-rights and resource checks. EBDT/EBLC, CBDT/CBLC, sbix and bdat/bloc
remain unsupported for bitmap rendering; their presence does not invalidate
usable TrueType outlines. Bitmap-only fonts remain unsupported. CFF/CFF2,
variable fvar and COLR/CPAL/SVG rejections retain their precedence, even when
glyf/loca records are present. No font/vendor special case was added.

Regression tests use synthetic table-presence sets and in-memory mutations of
the existing OFL O2P Test Sans fixture. No new binary fixture or proprietary font
was committed. Tests explicitly cover paired bitmap table sets, sbix, outline
precedence, incomplete outlines, invalid required table ranges, embedding rights,
unchanged shaping and the 256 registered/directory-font limits. All four O2P
fixtures continue to register through the existing directory-registration test.

## Manual Calibri validation

Each local PowerPoint DFonts face was supplied individually to the development
adapter, with no font directory: Calibri.ttf, Calibrib.ttf, Calibrii.ttf and
Calibriz.ttf. All four registered and rendered case 001 successfully (exit 0).
The original Calibri-only unicode-fonts reproduction also succeeds after the
fix. Proprietary fonts remain local and are not encoded into automated tests.

## Correctly paired 001-basic-shapes measurement

Input SHA-256: `bd2cad631d588fee1977d2a1a4b8d7a84d6a2c0c74b0a41b3e396f1e794cbac1`.
This matches the existing PowerPoint source.pptx. Reference: Microsoft PowerPoint
for Mac 16.113.3 (26092714), existing PDF-export PNGs at 144 DPI. Both pages have
959.976 × 540 point output and matching 1920 × 1080 raster dimensions.
Metric remains block-ssim-rgb-v1 with the existing thresholds.

Before values come from [FIDELITY_2C.json](FIDELITY_2C.json), after 2C/2B and
before exact fonts. A post-fix rerun with fixtures/fonts reproduced every
per-slide mean/worst-block value and the overall difference at full precision,
including the same 430 warnings. The exact-font run supplies only four explicit
files: Calibri regular/bold and Aptos regular/bold. No large font directory is used.

| Measurement | Fixture-font baseline | Exact Calibri/Aptos |
| --- | ---: | ---: |
| Slide 1 difference | 0.039127038948926426 | 0.020828237670597303 |
| Slide 1 worst block | 0.8223116471617005 | 0.4542546120228731 |
| Slide 2 difference | 0.032571536904083744 | 0.017536983148525365 |
| Slide 2 worst block | 0.8319984512058003 | 0.45806727149387255 |
| Overall difference | 0.03584928792650509 | 0.019182610409561332 |
| Worst block across slides | 0.8319984512058003 | 0.45806727149387255 |
| Warning occurrences | 430 | 338 |
| Both slides / case rating | MAJOR_DIFFERENCE | MINOR_DIFFERENCE |

Overall difference decreases **46.49%**. Native-resolution difference images
show reduced font-shape/width mismatch; text baseline/wrap displacement and thin
shape/shadow edges remain. Exact font availability improves fidelity without
changing text layout. Warning reduction is separate from the pixel result; the
remaining 338 warnings include four overflow warnings and unsupported semantics.

## Validation and local artifacts

- `cargo fmt` and `cargo fmt --check`: pass.
- `cargo clippy --workspace --all-targets --all-features`: pass.
- `cargo test --workspace`: 346 tests pass.
- Compatibility tooling unittest suite: 26 tests pass.
- `git diff --check`: pass.
- All 45 preserved reference, public corpus and font-fixture file hashes match.
- Cargo manifests/lockfile, font/resource ceilings and compatibility metric are unchanged.

The precedence regression was run before the policy fix and failed with the
reported bitmap error, then passed after the fix. Exact-font check exits 0;
fixture-font check exits 1 because MAJOR_DIFFERENCE is the expected rating.

Ignored `target/font-compatibility/` retains the baseline adapter, original
failure telemetry, baseline metrics/digests, individual Calibri-face PDFs and
telemetry, and fixture/exact-font comparison PDFs, PNGs and full JSON reports.
`repro-before.json`/`.stderr` retain the baseline bitmap rejection;
`repro.json` retains the successful rerun of the same reproduction.

To reproduce with your locally licensed font directory:

```sh
cargo build --offline -p o2p-cli --example compat_render
# Set OFFICE_FONT_DIR to your local PowerPoint DFonts directory.
compat/.venv/bin/python compat/scripts/compat.py check \
  compat/corpus/public/001-basic-shapes.pptx \
  --reference-dir compat/references/local/001-basic-shapes \
  --output-dir target/font-compatibility/exact-fonts \
  --adapter target/debug/examples/compat_render \
  --font "$OFFICE_FONT_DIR/Calibri.ttf" \
  --font "$OFFICE_FONT_DIR/Calibrib.ttf" \
  --font "$OFFICE_FONT_DIR/Aptos.ttf" \
  --font "$OFFICE_FONT_DIR/Aptos-Bold.ttf"
```
