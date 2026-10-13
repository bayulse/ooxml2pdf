# Fidelity 2C — Paragraph, bullet and textbox alignment

Baseline: clean committed `2b73b7b`. Completed 2026-10-13. No dependency changes.
All six current public PPTX files are byte-identical to the correctly paired local PowerPoint source.pptx files. References, input files, fonts and thresholds are unchanged. The historical Fidelity 2B public-input mismatch does not apply to this current corpus.

**All twelve slides improve their overall difference. 004 slide 2 improves 20.2%, from 0.128258 to 0.102387. Its worst block increases from 0.825514 to 0.857576. All six cases remain MAJOR_DIFFERENCE.**

Reference renderer: Microsoft PowerPoint for Mac 16.113.3 (26092714), existing PDF exports rasterized at 144 DPI. Explicit font directory: fixtures/fonts. Metric: unchanged block-ssim-rgb-v1; major worst-block threshold remains 0.60. Each input produces two 960 × 540 point pages (authored width rounds to 959.976pt) and two 1920 × 1080 PNGs. No missing comparisons or failed conversions.

## Per-slide pixels

| Case / slide | Difference before | Difference after | Worst block before | Worst block after | Rating before → after |
| --- | ---: | ---: | ---: | ---: | --- |
| 001-basic-shapes / 1 | 0.0448952903 | 0.0391270389 | 0.8233075144 | 0.8223116472 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 001-basic-shapes / 2 | 0.0616090454 | 0.0325715369 | 0.8388512126 | 0.8319984512 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 002-images / 1 | 0.0339997548 | 0.0257733110 | 0.8074939175 | 0.7811119325 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 002-images / 2 | 0.0323773415 | 0.0241137918 | 0.7944063440 | 0.7871337839 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 003-text / 1 | 0.0676664797 | 0.0609015424 | 0.8461905786 | 0.8554759953 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 003-text / 2 | 0.0715939497 | 0.0484838047 | 0.8564222752 | 0.8576249406 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 004-theme-layout / 1 | 0.0336144615 | 0.0258572657 | 0.8440567055 | 0.7955949393 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 004-theme-layout / 2 | 0.1282579803 | 0.1023868042 | 0.8255137949 | 0.8575758355 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 005-transforms / 1 | 0.0339350731 | 0.0218676207 | 0.8005179205 | 0.7999911388 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 005-transforms / 2 | 0.0326155995 | 0.0245070191 | 0.7796562173 | 0.7526331221 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 006-style-matrix / 1 | 0.0327870617 | 0.0248150152 | 0.7655048290 | 0.7717628249 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 006-style-matrix / 2 | 0.0298257948 | 0.0214217300 | 0.7274459087 | 0.6422432659 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |

## Overall case differences and warnings

| Case | Overall before | Overall after | Warnings before → after | Rating before → after |
| --- | ---: | ---: | ---: | --- |
| 001-basic-shapes | 0.0532521679 | 0.0358492879 | 581 → 430 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 002-images | 0.0331885481 | 0.0249435514 | 422 → 318 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 003-text | 0.0696302147 | 0.0546926736 | 541 → 407 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 004-theme-layout | 0.0809362209 | 0.0641220350 | 453 → 347 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 005-transforms | 0.0332753363 | 0.0231873199 | 524 → 386 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 006-style-matrix | 0.0313064283 | 0.0231183726 | 549 → 405 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |

## Typography diagnostics

These are parser-feature occurrences, not a count of visible bullets or an image acceptance criterion. Shared-source warnings are deduplicated by the existing source/feature rules.

| Case | Bullet properties | Vertical anchor | Paragraph spacing | Margin/indent | Autofit |
| --- | ---: | ---: | ---: | ---: | ---: |
| 001-basic-shapes | 19 → 0 | 55 → 0 | 45 → 0 | 27 → 0 | 37 → 37 |
| 002-images | 19 → 0 | 25 → 0 | 28 → 0 | 27 → 0 | 19 → 19 |
| 003-text | 19 → 0 | 41 → 0 | 42 → 0 | 27 → 0 | 32 → 32 |
| 004-theme-layout | 19 → 0 | 33 → 0 | 25 → 0 | 27 → 0 | 17 → 17 |
| 005-transforms | 19 → 0 | 49 → 0 | 38 → 0 | 27 → 0 | 30 → 30 |
| 006-style-matrix | 19 → 0 | 52 → 0 | 41 → 0 | 27 → 0 | 33 → 33 |
| **Total** | 114 → 0 | 255 → 0 | 219 → 0 | 162 → 0 | 168 → 168 |

Total warning occurrences: **3070 → 2293**. Supported buChar/buFont/buNone, paragraph margins/indent, spcBef/spcAft and t/ctr/b anchor warnings disappear because those properties now affect layout. Unsupported autofit remains **168 → 168** (156 spAutoFit, 12 normAutofit). Supported inherited bodyPr@vert=horz is recognized; unsupported writing modes still warn. Existing gradient/effect/script-font/metadata diagnostics remain.

On 004, two new Arial font diagnostics accompany actual bullet shaping (missing requested Arial and configured substitution); one additional configured-font text-box overflow diagnostic reflects the now-indented paragraph geometry. These are retained, not suppressed. Full feature-count additions/removals and full-precision metrics are in [FIDELITY_2C.json](FIDELITY_2C.json).

## Primary case: 004 slide 2, native-resolution inspection

- Bullets: four Unicode character bullets now appear. Arial requests follow the same configured font fallback as ordinary text; their circles remain slightly different from PowerPoint.
- Indentation: paragraph text and continuations move 27pt right while bullet origins remain at the inherited hanging position (marL=342900, indent=-342900). The missing bullet/text-margin displacement is substantially reduced.
- Wrapped continuations: align with paragraph text, not the bullet. Reference and rendered wrap points still differ: the final paragraph wraps before “breathing” in o2p but before “room” in PowerPoint because substituted font widths differ.
- Title alignment: existing inherited center paragraph alignment is preserved. Inherited centered vertical anchor moves the title down, improving vertical overlap; its apparent horizontal/width mismatch remains with substituted Aptos.
- Paragraph spacing: inherited 20% before spacing adds 6.4pt between 32pt body paragraphs; the first-edge gap is suppressed by the DrawingML default. Later paragraph baselines are closer to the reference.
- Vertical positioning: title centering improves; the body remains correctly top anchored. Insets and spacing contribute to the measured block before anchoring. Six swatches remain visually close and unchanged.

Both before/after difference images were inspected at their original 1920 × 1080 resolution, as were the reference and rendered slide. Text outline displacement decreases at bullet margins and paragraph baselines, but font-size/shape/advance and final-line wrapping differences remain. The color swatches show no regression. No gradients, font shrinking, autofit or corpus-specific coordinates were introduced.

Native unblurred RGB mean absolute error on slide 2 (additional descriptive measure, no threshold changes): **0.0511730280 → 0.0442041708**, a 13.6% improvement. The official acceptance measurement remains the unchanged compatibility metric.

Native difference images: [before](../../target/fidelity2c/before-images/004-theme-layout-c644edfb/run-n37wdn0k/difference-2.png), [after](../../compat/outputs/public/004-theme-layout-c644edfb/run-hthz9u27/difference-2.png).

## Implementation and deferred work

PPTX parses/resolves sparse paragraph and body semantics. Core owns generic bullet/font requests, paragraph geometry/spacing and vertical alignment. Layout shapes Unicode bullets through fonts, wraps within effective text margins, adds paragraph spacing boxes and anchors the measured block. Both configured and fixed-font paths delegate to generic layout. PDF receives only positioned text and existing transform scopes. [TYPOGRAPHY.md](../../docs/TYPOGRAPHY.md) documents precedence, units, defaults, bounds and unsupported forms.

The bullet source is master txStyles/bodyStyle/lvl1pPr, not direct slide pPr. The layout only selects levels; slide paragraphs leave properties absent. The title uses master titleStyle center alignment and master placeholder bodyPr center anchor. Fifteen original synthetic fixtures and tests cover Unicode/custom fonts/fallback, overrides/no-bullet, inherited indentation/alignment/spacing, wrapped continuations, edge spacing, points/percentages, textbox insets and rotation.

Full spAutoFit/normAutofit is deferred. Corpus usage is documented from all ZIP parts, including unused layouts; spAutoFit requests resizing the shape to text, while normAutofit can scale text/line spacing. The renderer preserves authored fonts/geometry, warning on both. Existing 1.2-times-font-size line metrics remain unchanged. Missing Aptos/Arial, font metric line boxes, line spacing and wider typography remain fidelity limits. Gradients/patterns/additional effects/shapes/groups/connectors/arrows/charts/SmartArt/animations/tables/DOCX/XLSX/Flutter are outside 2C.

## Validation and artifacts

- cargo fmt: pass.
- cargo clippy --workspace --all-targets --all-features: pass without warnings.
- cargo test --workspace: **340 tests pass**, including 25 Fidelity 2C parser/PDF tests and nine added generic-layout tests.
- Compatibility tooling: **26 tests pass**. The stale generator-versus-PowerPoint-input assertion now verifies generation determinism in separate temporary directories and validates actual public packages without changing them.
- Required six-case batch: complete; six MAJOR_DIFFERENCE, no failed render or missing reference. Expected harness exit status 1.
- SHA-256 audit: all reference files, all public PPTX bytes and all pre-existing fixture bytes unchanged. Each public PPTX still equals its reference source.pptx.
- git diff --check and cargo fmt --check: pass.

The pre-existing cargo example filename collision warning for the two generate_fixtures examples remains during workspace tests; it does not fail tests. No new compiler/clippy warnings remain.

Before adapter, reports, image artifacts, digest manifest, checks and the report-generation script are retained in ignored target/fidelity2c. Final required batch results/images are in compat/reports/public and compat/outputs/public; final snapshots are preserved under target/fidelity2c/final-reports and final-images. Stable comparative data is included as this report and its JSON companion. Changes remain in the working tree for review.

```sh
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
compat/.venv/bin/python -m unittest discover -s compat/scripts -p 'test_*.py'
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public \
  --reference-root compat/references/local --font-dir fixtures/fonts
```

Cargo checks used the populated cache with --offline. The preserved baseline adapter was built before editing and used the same batch with --adapter target/fidelity2c/baseline-render, --output-dir target/fidelity2c/before-images and --report-dir target/fidelity2c/before. Neither references nor thresholds were regenerated or edited.
