# Fidelity 2B — Rounded geometry and basic outer shadows

Baseline: clean committed `2a39e16` (Fidelity 2A). Implementation and measurement: 2026-10-13.

**The required public-input batch has zero visual improvement. The separate correctly paired PowerPoint-input batch improves all twelve slides. All six cases in both batches remain MAJOR_DIFFERENCE.**

## Input/reference identity finding

The generated PPTX files in `compat/corpus/public` are different presentations from the PPTX files in `compat/references/local` that produced the reference PNGs. Matching case names do not establish matching document identity. XML inspection found no `roundRect` or `outerShdw` anywhere in the generated public inputs. Their effect references (cases 004 and 006) are zero; theme effect lists have no shadows.

The local PowerPoint source presentations contain rounded cards/buttons and circular shadows. Case 001 uses `roundRect` with `a:gd name="adj" fmla="val 12000"`. Theme effect entries 1/2 supply downward outer shadows: `blurRad=40000`, `dist=20000/23000`, `dir=5400000`, `rotWithShape=0`, black with alpha 38000/35000. They are theme/style referenced, not direct outer-shadow definitions on slides. The baseline rejects rounded shapes rather than drawing them as rectangles; nonempty theme effects are unsupported.

The requested batch is retained unchanged. For meaningful feature validation, the baseline adapter was built/copied before editing, then both adapters rendered the actual local PowerPoint PPTX files against their existing reference PNGs. Both batches use the same font directory (`fixtures/fonts`), 144 DPI, reference provenance and thresholds. Reference inputs are read-only; none was regenerated or modified.

## Required public batch: before → after

| Case | Slide 1 | Slide 2 | Overall | Warnings | Rating |
| --- | ---: | ---: | ---: | ---: | --- |
| 001-basic-shapes | 0.239340 → 0.239340 | 0.168438 → 0.168438 | 0.203889 → 0.203889 | 3 → 3 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 002-images | 0.260955 → 0.260955 | 0.177555 → 0.177555 | 0.219255 → 0.219255 | 3 → 3 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 003-text | 0.355117 → 0.355117 | 0.166554 → 0.166554 | 0.260836 → 0.260836 | 4 → 4 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 004-theme-layout | 0.603106 → 0.603106 | 0.194066 → 0.194066 | 0.398586 → 0.398586 | 4 → 4 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 005-transforms | 0.163822 → 0.163822 | 0.202146 → 0.202146 | 0.182984 → 0.182984 | 3 → 3 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 006-style-matrix | 0.303301 → 0.303301 | 0.620857 → 0.620857 | 0.462079 → 0.462079 | 3 → 3 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |

Case 001 retains its committed baseline: slide 1 **0.239340**, slide 2 **0.168438**, overall **0.203889**. Every public slide/case value is exactly unchanged at full stored precision. Total public warnings: **20 → 20**; no new public warnings. No render failures or missing comparisons.

## Supplemental paired PowerPoint-source batch: before → after

| Case | Slide 1 | Slide 2 | Overall | Warnings | Rating |
| --- | ---: | ---: | ---: | ---: | --- |
| 001-basic-shapes | 0.152951 → 0.044895 | 0.074283 → 0.061609 | 0.113617 → 0.053252 | 593 → 581 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 002-images | 0.041996 → 0.034000 | 0.045482 → 0.032377 | 0.043739 → 0.033189 | 423 → 422 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 003-text | 0.290719 → 0.067666 | 0.079697 → 0.071594 | 0.185208 → 0.069630 | 547 → 541 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 004-theme-layout | 0.087243 → 0.033614 | 0.157904 → 0.128258 | 0.122573 → 0.080936 | 452 → 453 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 005-transforms | 0.068877 → 0.033935 | 0.102846 → 0.032616 | 0.085862 → 0.033275 | 528 → 524 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 006-style-matrix | 0.145771 → 0.032787 | 0.175481 → 0.029826 | 0.160626 → 0.031306 | 549 → 549 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |

The paired case 001 overall difference decreases by **53.1%**; case 006 decreases by **80.5%**. These gains cannot be substituted into the historical public-input table: they use different inputs. All twelve paired PDF pages render successfully. Both batches preserve page counts and accepted point/raster dimensions.

## Warnings and new diagnostics

Paired before total warnings: **3092**.
Paired after total warnings: **3070**.

Warnings are not the success metric. Newly visible paired-source diagnostics include specific unsupported theme `scene3d`/`sp3d` siblings (replacing the earlier coarse effect-style warning), unsupported line shadows, and text shadows on shapes without a painted silhouette. Newly rendered rounded text-bearing shapes also expose existing `bodyPr@rtlCol`, font substitution/missing Aptos and overflow diagnostics; text-box indices can change as skipped shapes become renderable. No text/color inheritance cleanup was performed. These remain structured/source-scoped warnings; supported outer shadows are preserved independently of unsupported effects. The public batch has no new diagnostic categories.

## Native-resolution visual inspection

Inspected native 1920 × 1080 difference images and rendered pages, especially both case 001 slides and case 006 cards/buttons. In the paired run:

- Card corners: visibly smaller differences; missing rounded geometry is restored. The broad case 001 card/banner fill discrepancy disappears.
- Button/banner corners: visibly smaller differences around rounded components, including case 006 swatches/cards and case 001 bottom banners.
- Card shadows: the strong missing-shadow edge differences decrease; thin residual rings remain.
- Circular shadows: the strong bottom-edge arcs under case 001 circles decrease; some blur-edge differences remain.

The required public batch images remain unchanged. Remaining paired differences include text/font/layout and thin shadow edges; the three-box blur is an approximation of PowerPoint blur. No claim of pixel identity or upgraded case rating is made. Line and text shadows remain unsupported by design.

## Implementation and validation

Generic rounded fill/outline operations remain vector PDF paths. Generic local shadows support rectangle, rounded rectangle, ellipse and source-image alpha silhouettes. PPTX owns preset guides, units, angles, color/theme resolution and direct/inherited/effect-reference precedence. Local empty effects suppress inherited/theme effects. Blur uses bounded individual masks (2048 pixels/side, 1048576 pixels total), adaptive resolution and linear-time separable filtering; no full-slide rasterization or dependency changes. See [EFFECTS.md](../../docs/EFFECTS.md).

Eleven original synthetic fixtures reproduce byte-for-byte. New parser/backend tests cover radius, fill/no-fill/stroke, text/transforms, direct/theme/local precedence, inheritance, ordering, vector operators, alpha masks, transparent images, limits and malformed/extreme inputs. Historical unsupported-effect tests now exercise the still-unsupported inner shadow; malformed supported outer shadows are covered by the new tests.

- `cargo fmt` and final format check: pass.
- `cargo clippy --workspace --all-targets --all-features --offline`: pass, no clippy warnings.
- `cargo test --workspace --offline`: 306 tests pass (including doctests).
- Compatibility tooling tests: 26 pass.
- Required six-case public comparison: complete; expected exit 1 for MAJOR_DIFFERENCE.
- Supplemental six-case paired comparison: complete; expected exit 1 for MAJOR_DIFFERENCE.
- `git diff --check`: pass.
- All 30 local reference files match the preserved Fidelity 2A SHA-256 manifest. Corpus inputs, reference files, metric thresholds, Cargo dependencies and existing fixture bytes are unchanged.

Cargo test retains the pre-existing same-name `generate_fixtures` example output collision warning between o2p-ooxml and o2p-pptx; it does not fail compilation or tests.

## Commands and review artifacts

```sh
cargo build --offline -p o2p-cli --example compat_render
# Pre-edit adapter was copied to target/fidelity2b/baseline-render.
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public \
  --reference-root compat/references/local --font-dir fixtures/fonts
compat/.venv/bin/python compat/scripts/compat.py batch compat/references/local \
  --reference-root compat/references/local --font-dir fixtures/fonts \
  --adapter target/fidelity2b/baseline-render \
  --output-dir target/fidelity2b/paired-before-images \
  --report-dir target/fidelity2b/paired-before
# Repeat the paired command with the current adapter and paired-after paths.
```

Detailed JSON (full precision, source identities, slides, warnings), build/check logs, baseline adapter and paired image directories are preserved under ignored `target/fidelity2b`. Current public reports/images remain under ignored `compat/reports/public` and `compat/outputs/public`. The files below identify the two different PPTX inputs for each case.

| Case | Public input SHA-256 | PowerPoint reference input SHA-256 |
| --- | --- | --- |
| 001-basic-shapes | `42f43122405c380e8ff25cfde3bdbfc1cc53760864581dc0b915934598d4c2fd` | `bd2cad631d588fee1977d2a1a4b8d7a84d6a2c0c74b0a41b3e396f1e794cbac1` |
| 002-images | `f3de171ad4e05496e883ce0520c5fec54e1d65f9e1a0f246bd0e385622e6318f` | `6f4c82403bca3358ceb62b2bd2631e36fdead8c5cf73d0a8981f755233062314` |
| 003-text | `0fb927ebe60746263f10e2a52a8fee3fe75d6f7d1b453c914932d81513f37927` | `f327d67116a6c383ce3ed3787db2a94065a515f7d49f7c76a8240577a338e619` |
| 004-theme-layout | `60fd5ccd3b61859677ea5e9a33e0612f0cf6c9ae9df667c63918f0c4e8743f18` | `a77cd7a020dc42352ea1a049053424878356dbf8d4fed7bea5a595853c4f5b17` |
| 005-transforms | `7f69defb5c1b9470966b765ba8b344bc76abeb53b2c85d6f0a184b558b53efac` | `a918e006415a58b2c3809bf2d4be3fe01f4ed71192cea29db5b8d4b72fb7f94d` |
| 006-style-matrix | `d3265703247d671b136f73dea11d2c2fba5431bef4cf9d470b5630329df613c7` | `bfc9a0f105debf7c49e6b2aa6590e981b7418471ba7341891026b5da184998bc` |
