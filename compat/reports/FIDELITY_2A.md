# Fidelity 2A — Text inheritance and color-map semantics

Completed 2026-10-13. Code baseline: clean committed `d1a27f6`.
The preserved independent baseline reports record renderer commit `7ffe660`;
`git diff 7ffe660 d1a27f6 -- crates Cargo.toml Cargo.lock` is empty. Their renderer
code is therefore the same as the clean starting state. Before reports/telemetry
were saved before implementation; current reports use the Fidelity 2A working tree.

Reference renderer: Microsoft PowerPoint for Mac Version 16.113.3 (26092714),
PDF export followed by Poppler rasterization. Existing references only; no
PowerPoint exports, reference files, corpus inputs, or metric thresholds changed.
Configured fonts: `fixtures/fonts`; PNG rasterization: 144 DPI.

**Measured result: no corpus image-metric improvement. All six cases remain
MAJOR_DIFFERENCE.** Supported semantics and diagnostics improve, but warning
removal is not a visual fidelity claim.

## 001-basic-shapes

| Measurement | Before | After |
| --- | ---: | ---: |
| Slide 1 difference | 0.239340142357 | 0.239340142357 |
| Slide 2 difference | 0.168438001812 | 0.168438001812 |
| Overall difference | 0.203889072085 | 0.203889072085 |
| Total warnings | 17 | 3 |
| Text fallback warnings | 10 | 0 |
| clrMap warnings | 1 | 0 |
| clrMapOvr warnings | 3 | 0 |
| Rating | MAJOR_DIFFERENCE | MAJOR_DIFFERENCE |

## All six cases

Differences below are paired-slide/overall values from the unchanged metric.
All before/after comparisons are exactly equal at full stored precision, not just
at the displayed rounding. Each case has two successfully rendered PDF pages.

| Case | Slide 1 before → after | Slide 2 before → after | Overall before → after | Warnings before → after | Text fallback before → after |
| --- | --- | --- | --- | --- | --- |
| 001-basic-shapes | 0.239340 → 0.239340 | 0.168438 → 0.168438 | 0.203889 → 0.203889 | 17 → 3 | 10 → 0 |
| 002-images | 0.260955 → 0.260955 | 0.177555 → 0.177555 | 0.219255 → 0.219255 | 15 → 3 | 8 → 0 |
| 003-text | 0.355117 → 0.355117 | 0.166554 → 0.166554 | 0.260836 → 0.260836 | 17 → 4 | 9 → 0 |
| 004-theme-layout | 0.603106 → 0.603106 | 0.194066 → 0.194066 | 0.398586 → 0.398586 | 14 → 4 | 7 → 1 |
| 005-transforms | 0.163822 → 0.163822 | 0.202146 → 0.202146 | 0.182984 → 0.182984 | 16 → 3 | 9 → 0 |
| 006-style-matrix | 0.303301 → 0.303301 | 0.620857 → 0.620857 | 0.462079 → 0.462079 | 14 → 3 | 7 → 0 |

All six before/after ratings: **MAJOR_DIFFERENCE → MAJOR_DIFFERENCE**.
Total warnings: **93 → 20**. Text fallback: **50 → 1**. Master clrMap:
**6 → 0**. Layout/slide clrMapOvr: **18 → 0**. No cases fail to render or lack
references. Page count, point dimensions, raster dimensions, and reference
provenance remain valid.

## Evidence and interpretation

The previous text fallback was emitted whenever a textbox lacked placeholder
defaults, including ordinary text with complete direct run/paragraph formatting.
The resolver also missed master txStyles, levels above zero, and inherited insets.
Fidelity 2A implements those paths and checks the properties actually unresolved
at the end. It does not remove the warning indiscriminately.

None of the six public PPTX inputs contains p:txStyles. Ordinary corpus text
already supplies local family/size/color, while its scheme usages are concrete
dk1 and identity-mapped accents. Their inherited RGB paints therefore stay the
same after generic clrMap implementation. The corpus does not visually exercise
the changed inheritance/remapping paths. Nineteen original synthetic fixtures
and focused tests exercise them without tuning to reference pixels.

The sole remaining text fallback is 004-theme-layout slide 1's body placeholder:
local rPr supplies 18pt and O2P Test Sans, but no local/layout/master color or
master text style resolves color. It now warns specifically about text color;
previously an empty placeholder-default record suppressed that diagnosis.
Remaining warnings: notesSz (6), sldMasterIdLst (6), sldLayoutIdLst (6), legacy
unwrapped overflow (1), unresolved text color (1). They remain visible.

The milestone adds no shadows, gradients, groups, charts, SmartArt, rounded
rectangles, connectors/arrows, effects, animations, DOCX/XLSX, or Flutter features.
Presentation defaultTextStyle, script-specific font selection, bullet rendering,
spacing, kerning, autofit, vertical writing, and non-top anchoring remain deferred
with structured warnings. No dependency changes.

## Validation

- cargo fmt and git diff --check: passed.
- cargo clippy --workspace --all-targets --all-features: passed without clippy warnings.
- cargo test --workspace: passed; 288 tests, including 25 Fidelity 2A tests.
- Compatibility tooling: all 26 unittest tests passed.
- Six-case compatibility batch: completed, 6 MAJOR_DIFFERENCE, no conversion errors.
  Exit 1 is the harness's expected result for MAJOR_DIFFERENCE.
- Reference integrity: all 30 existing reference files have identical SHA-256 digests
  before and after; the file set is unchanged.
- Existing fixture regeneration did not change any previously committed fixture.
- All new fixtures reproduce their generated ZIP bytes; PDFs render deterministically.
  Unicode extraction/ToUnicode and existing font/wrapping/transform regressions pass.

Cargo checks used the populated local cache with --offline; no runtime/network
or third-party dependencies were added. Cargo test still reports the pre-existing
same-name generate_fixtures example output collision between o2p-ooxml and
o2p-pptx; compilation and tests pass.

```sh
cargo fmt
cargo clippy --workspace --all-targets --all-features --offline
cargo test --workspace --offline
compat/.venv/bin/python -m unittest discover -s compat/scripts -p 'test_*.py'
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public \
  --reference-root compat/references/local --font-dir fixtures/fonts
```

Detailed run reports remain in ignored compat/outputs/public and
compat/reports/public. Preserved before/after JSON, reference digests, and check
logs are in ignored target/fidelity2a. Current public API/precedence and limitations
are documented in docs/INHERITANCE.md and docs/THEMES.md.
