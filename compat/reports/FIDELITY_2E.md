# Fidelity 2E — Font vertical metrics and baseline parity

**Complete: 6/6 paired cases MINOR_DIFFERENCE.** All twelve slide means improve; eleven worst blocks improve and one is unchanged. 004 slide 2 passes the existing 0.60 worst-block bound. No thresholds, references, inputs, dependencies or font assets changed.

## Baseline and provenance

Measured baseline is the preserved pre-edit Fidelity 2D working tree, initially on 11bd2e4. User subsequently committed 2D as 441552f; that commit omits the existing lib.rs/registry.rs 2D integration, which is retained in the working tree. No history was rewritten by the agent.

The preserved adapter reproduces every full-precision Fidelity 2D slide metric. Both adapters receive the same sixteen local Office files by repeated `--font` flags in filename order. Basenames/hashes, table values, unchanged metric parameters and all numerical results are in [FIDELITY_2E.json](FIDELITY_2E.json). PowerPoint reference PDFs/PNGs and the correctly paired input presentations remain unchanged. Rasterization is 144 DPI.

## Numerical title trace: 004 slide 2

Slide and slideLayout2 title placeholders both omit geometry. The master supplies x=457200, y=274638, cx=8229600, cy=1143000 EMU (12700 EMU/pt). The parsed effective shape matches this geometry. Insets inherit 91440/45720 EMU (7.2/3.6pt); master anchor remains center. The title has no direct run formatting: master titleStyle supplies 44pt, +mj-lt resolves to Aptos Display, and paragraph alignment is center. No geometry or inheritance correction was needed.

| Measurement (points) | Before | After |
| --- | ---: | ---: |
| Shape y | 21.625040054 | 21.625040054 |
| Shape height | 90.000000000 | 90.000000000 |
| Inner y | 25.225040436 | 25.225040436 |
| Inner height | 82.800003052 | 82.800003052 |
| Line count | 1.000000000 | 1.000000000 |
| Line height | 52.800000000 | 53.710937500 |
| Block height | 52.800000000 | 53.710937500 |
| Line top after anchoring | 40.225041962 | 39.769573212 |
| Baseline offset from line top | 44.000000000 | 41.314453125 |
| Final first-line baseline/origin y | 84.225036621 | 81.084022522 |
| Final first-line origin x | 88.856430054 | 88.856430054 |
| Line width | 542.287109375 | 542.287109375 |

Top/bottom insets: 3.6pt each. Font: Aptos Display regular, 44pt. One line; zero retained paragraph edge spacing; anchor: Center. The before path used baseline=top+44 and line height=52.8. It exposed font metadata but never used it in layout. This affects baseline, line advance, block height and anchor offsets. Paragraph percentage spacing incorrectly inferred font size from line height/1.2; 2E now stores text size independently.

The corrected model scales ascent=1923/2048×44=41.314453125pt, descent=577/2048×44=12.396484375pt, gap=0; height=53.7109375pt. The center calculation moves the baseline upward about 3.141016pt, or 6.282031 pixels at 144 DPI. The measured remaining raster displacement is about 1px. There is no hardcoded raster or font-family offset.

## Available OpenType metrics and selection

| Local font | unitsPerEm | hhea A / D / G | OS/2 typo A / D / G | Win ascent / descent | USE_TYPO_METRICS |
| --- | ---: | --- | --- | --- | --- |
| AptosDisplay-Regular.ttf | 2048 | 1923 / -577 / 0 | 1923 / -577 / 0 | 2068 / 563 | True |
| Aptos.ttf | 2048 | 1923 / -577 / 0 | 1923 / -577 / 0 | 2068 / 563 | True |
| Arial.ttf | 2048 | 1854 / -434 / 67 | 1491 / -431 / 307 | 1854 / 434 | False |
| Calibri.ttf | 2048 | 1950 / -550 / 0 | 1536 / -512 / 452 | 1950 / 550 | False |

The preceding ttf-parser convenience getters selected typo when its version-aware bit was set, otherwise normally hhea, with per-field fallbacks for zeros. Layout ignored all three exposed values. Aptos Display has identical hhea/typo triplets, so changing between those tables cannot explain this title offset; the nominal baseline heuristic does. Win metrics differ and are not selected when USE_TYPO_METRICS is set.

2E explicitly selects complete triplets: version >=4 plus bit 7 selects OS/2 typo; otherwise valid hhea; then complete typo; then unsigned Win with zero gap. Invalid authoritative typo metrics return an owned error. Selection uses metadata, never family/vendor names. Negative gap normalizes to zero. This is a deterministic portable policy; the corpus does not prove which table every PowerPoint version uses for every unflagged font. [OpenType OS/2](https://learn.microsoft.com/en-us/typography/opentype/spec/os2), [hhea](https://learn.microsoft.com/en-us/typography/opentype/spec/hhea).

## Generic line model and autofit boundary

Fonts owns design-unit metadata, point normalization and ShapeReport metric maxima over actual resolved grapheme faces. Layout carries metrics beside shaped fragments and takes independent line ascent/descent/gap maxima across mixed font/size/style/fallback runs and the first-line bullet. Baseline=line top+ascent; line bottom=baseline+descent+gap. Leading follows descent. Blank lines use the resolved end font/size; empty documents without configured fonts retain zero unpainted metrics. Largest text size remains separate for paragraph percentage spacing. Wrapping, advances, colors/styles, source clusters, Unicode shaping and PDF layout boundaries remain intact.

Block height sums line heights and paragraph spacing. Top begins at inner top; center at inner top+(inner height−block height)/2; bottom at inner bottom−block height. PPTX resolves body/inheritance/insets and translates generic requests; PDF only serializes the positioned baselines. The legacy unconfigured standard-font path retains its nominal metrics.

The title master has empty normAutofit. Omitted fontScale implies 100000 (100%) and lnSpcReduction implies 0 (0%): [fontScale semantics](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit.fontscale), [spacing-reduction semantics](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.normalautofit.linespacereduction). The authored title fits the inner box with these scalable metrics. No evidence requires iterative fitting here. Autofit warnings remain; no general normAutofit/spAutoFit, kerning, character spacing, gradient or other excluded feature was added.

## All six case metrics

| Case | Overall before | Overall after | Warnings before → after | Rating before → after |
| --- | ---: | ---: | ---: | --- |
| 001-basic-shapes | 0.019182610410 | 0.012769951303 | 338 → 341 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 002-images | 0.015889994521 | 0.008304393234 | 271 → 271 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 003-text | 0.024041569263 | 0.013981363535 | 320 → 323 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 004-theme-layout | 0.030199395815 | 0.021075697166 | 297 → 297 | MAJOR_DIFFERENCE → MINOR_DIFFERENCE |
| 005-transforms | 0.014330365216 | 0.009148928924 | 314 → 314 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 006-style-matrix | 0.012036478980 | 0.008528498539 | 325 → 325 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |

## Every slide: before / after

| Case / slide | Difference before | Difference after | Worst block before | Worst block after | Rating before → after |
| --- | ---: | ---: | ---: | ---: | --- |
| 001-basic-shapes / 1 | 0.020828237671 | 0.013556152185 | 0.454254612023 | 0.454254612023 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 001-basic-shapes / 2 | 0.017536983149 | 0.011983750420 | 0.458067271494 | 0.245268129600 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 002-images / 1 | 0.016548268671 | 0.008876923409 | 0.393896588734 | 0.226895854812 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 002-images / 2 | 0.015231720371 | 0.007731863059 | 0.373278044692 | 0.237910691321 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 003-text / 1 | 0.025578169675 | 0.014402816607 | 0.467925712620 | 0.236370550206 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 003-text / 2 | 0.022504968852 | 0.013559910463 | 0.393829798174 | 0.345319284962 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 004-theme-layout / 1 | 0.012996057842 | 0.007215893633 | 0.470726988323 | 0.201521433854 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 004-theme-layout / 2 | 0.047402733788 | 0.034935500699 | 0.755559818049 | 0.445491098945 | MAJOR_DIFFERENCE → MINOR_DIFFERENCE |
| 005-transforms / 1 | 0.012356903275 | 0.008460658267 | 0.371171626522 | 0.230151369863 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 005-transforms / 2 | 0.016303827157 | 0.009837199580 | 0.338832437721 | 0.226860294551 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 006-style-matrix / 1 | 0.014194471183 | 0.009975132605 | 0.366847579078 | 0.231366891195 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 006-style-matrix / 2 | 0.009878486776 | 0.007081864473 | 0.387308808659 | 0.185897523411 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |

## Title image inspection

Native 1920×1080 rendered slides and difference images were inspected. The broad doubled title edges largely collapse to thin residual contours; body text retains multiline baseline differences. No further feature was implemented. Dark-ink bounds below use the same crop and max-channel<128 threshold before/after, with right/bottom exclusive. Antialiasing thresholds can change an edge by one pixel.

| Image | Left | Top | Right | Bottom |
| --- | ---: | ---: | ---: | ---: |
| reference | 185 | 103 | 1257 | 176 |
| before | 184 | 110 | 1258 | 183 |
| after | 184 | 104 | 1258 | 177 |

Horizontal displacement: left −1px / right +1px relative to PowerPoint, unchanged before/after. Vertical displacement: top and bottom +7px → +1px. The native title width remains unchanged; no horizontal policy changed. Official slide metrics: difference 0.04740273378830124 → 0.03493550069883729; worst block 0.755559818049309 → 0.4454910989447289.

[before actual image](../../target/fidelity2e/before/004-theme-layout-c644edfb/run-44bi97ku/slide-2.png)
[before difference image](../../target/fidelity2e/before/004-theme-layout-c644edfb/run-44bi97ku/difference-2.png)
[after actual image](../../target/fidelity2e/after/004-theme-layout-c644edfb/run-nmsnfehf/slide-2.png)
[after difference image](../../target/fidelity2e/after/004-theme-layout-c644edfb/run-nmsnfehf/difference-2.png)

## Validation and reproduction

- cargo fmt / fmt --check: pass.
- cargo clippy --workspace --all-targets --all-features: pass (offline cache).
- cargo test --workspace: 369 tests pass (offline cache).
- All compatibility tooling tests: 27 pass.
- Generic synthetic metric tests cover upem/ascent/descent/gap normalization, metric source/version, invalid values, complete-triplet fallbacks and unsigned Windows ranges. Existing OFL outlines are reused; no Microsoft fonts added.
- Generic line/anchor tests cover top/center/bottom, multiline, inset rectangles, spacing, mixed fonts/sizes, per-grapheme fallback, blank end-fonts and bullets. A vertical-table-only mutation preserves glyphs/offsets/advances/line breaks/widths/x alignment exactly.
- Existing 2D aliases and deterministic ties, bitmap-strike compatibility, bold/italic, Arabic/Japanese, source-cluster safety and PDF extraction tests pass. Existing numeric baseline tests now assert scalable values; no assertions were weakened to accept an arbitrary offset.
- All 212 input/reference/fixture/font hashes and file sets unchanged. Paired source hashes match every public input. Every baseline slide metric matches the 2D report exactly.
- git diff --check: pass. Cargo manifests/lockfile and thresholds unchanged.

```sh
cargo fmt
cargo clippy --offline --workspace --all-targets --all-features
cargo test --offline --workspace
compat/.venv/bin/python -m unittest discover -s compat/scripts -p 'test_*.py'
cargo build --offline -p o2p-cli --example compat_render --example text_layout_trace
compat/.venv/bin/python target/fidelity2e/run.py before
compat/.venv/bin/python target/fidelity2e/run.py after
```

The ignored local driver registers the sixteen font paths explicitly, keeps the preserved before adapter, and checks all six cases against existing local references. On another installation use the documented compat.py commands with the same named faces. `text_layout_trace input.pptx --font file ...` provides numerical diagnosis; normal conversion emits no new logging. Full local trace is in ignored `target/fidelity2e/title-trace-after.txt`.
