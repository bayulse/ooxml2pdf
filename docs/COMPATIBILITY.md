# PPTX compatibility and fidelity evaluation

The rendering foundation at `v0.1.0-alpha.4` / `82e9e8a` is the initial baseline.
This milestone measures existing behavior; it adds no rendering features.

The pipeline is PPTX → existing o2p conversion → PDF → Poppler PNGs → comparison
with separately rendered slide images → JSON and Markdown reports. Rendering
warnings remain unchanged and are aggregated only by the evaluation tooling.

## Corpus philosophy

Keep small original, reproducible compositions alongside private real-world
decks. Unit fixtures isolate parser behavior; compatibility decks combine text,
shapes, images, backgrounds, themes and inheritance into complete slides. Six
original public decks live in `compat/corpus/public`; each has two slides. Generate
them with `python3 compat/scripts/generate_corpus.py`. Do not treat this small
corpus as representative of every Office document.

After diagnosing a real failure, create a minimal public reproduction with
original content if possible. Record the renderer, fonts, slide size and export
settings. Preserve a regression fixture before implementing a renderer fix.

## Public and private data

```text
compat/
  corpus/public/          tracked original PPTX cases
  corpus/private/         ignored proprietary inputs
  corpus/real-private/    ignored alternative private input location
  references/local/      ignored local references for public cases
  references/private/    ignored private references
  outputs/public/        ignored generated PDFs, PNGs and per-case reports
  outputs/private/       ignored private artifacts and per-case reports
  reports/public/        ignored generated batch and fidelity-gap reports
  reports/private/       ignored private batch and fidelity-gap reports
  scripts/               runner, generator, tests, Python requirements
```

Inputs outside `compat/corpus/public` default to private output/report directories.
Explicit output options override that policy: use ignored paths for private data.
Reports include full input paths, hashes, warning messages and source locations;
they can be sensitive even without the input file. Nothing is uploaded. The
tooling never invokes PowerPoint or LibreOffice automatically.

Do not commit proprietary PPTX files or Office-generated references without clear
redistribution permission. The existing tracked synthetic unit fixtures are not
ignored. Reference metadata may be shared separately only after review.

## Development dependencies and setup

Python 3.10+ and Pillow 12.3.0 provide PNG loading, resampling, blur and diagnostic
difference images. Pillow is a maintained development-only dependency under the
permissive [MIT-CMU license](https://pillow.readthedocs.io/en/stable/about.html).
The SSIM-style calculation is implemented here without NumPy or scikit-image.

Poppler's `pdftoppm` and `pdfinfo` are optional development executables required
only to run evaluation. Poppler is GPL software invoked as a separately installed
tool; it is not linked, bundled, or added to Cargo. This use is explicitly within
the development-rasterizer scope of the compatibility milestone. Missing tools
produce an actionable error and a failure report. The native runtime libraries
remain offline, contain no external-process dependencies and have unchanged APIs
and dependency graphs. No Cargo dependency or lockfile change is needed.

```sh
# macOS
brew install poppler
# Debian/Ubuntu
sudo apt install poppler-utils python3-venv

python3 -m venv compat/.venv
compat/.venv/bin/python -m pip install -r compat/scripts/requirements.txt
cargo build --offline -p o2p-cli --bin o2p --example compat_render
```

The runner builds `compat_render` automatically using the local Cargo cache when
`--adapter` is omitted. Build dependencies must already be fetched for offline
Cargo builds. A custom Cargo target directory can be used by passing the adapter
path explicitly. `compat_render` is an example target inside `o2p-cli`, using the
same parser, font layout, DisplayList and PDF report APIs as `o2p convert`.
It exports warning enum kinds, feature names and locations without parsing
human-readable stderr. It is never linked into `o2p` or a library. Conversion
parity with the regular CLI is tested byte-for-byte. Font options retain the CLI's
explicit configuration: no automatic font discovery or implicit fixture fonts.

## Generating PowerPoint reference images locally

1. Open the PPTX in Microsoft PowerPoint. Inspect any repair message first; a
   repaired document must be recorded as such rather than silently replacing the
   input. Install the project test fonts for public corpus comparisons, or use the
   same licensed fonts supplied to o2p for a private deck.
2. Export **all slides** as PNG images into an ignored case directory, for example
   `compat/references/local/001-basic-shapes/`. Use slide images, not notes,
   handouts, thumbnails or presentation screenshots. Check image count and order.
3. Record PowerPoint version/build, operating system, font setup, export settings
   and any substitutions or repairs. Obtain physical slide size from PowerPoint's
   slide-size dialog: inches × 72 = points; centimeters × 72 / 2.54 = points.
4. Create metadata, then verify its ordered `pages` list:

```sh
compat/.venv/bin/python compat/scripts/compat.py reference \
  compat/references/local/001-basic-shapes \
  --renderer 'Microsoft PowerPoint' --version 'record actual version/build' \
  --width-pt 960 --height-pt 540 \
  --notes 'Record OS, PNG export settings, installed O2P fonts, repairs/substitutions'
```

The manifest command naturally sorts filenames (`Slide2.png` before
`Slide10.png`) and prints the order. It refuses to overwrite existing provenance.
Edit the manifest explicitly if filenames do not imply the correct slide order.
Each page has `image`, `width_pt`, `height_pt`; the manifest includes `renderer`,
`version`, `notes`, `created_utc`. Physical size is mandatory because PNG pixel
dimensions alone do not establish slide dimensions. Referenced images must exist,
be unique and resolve inside the reference directory.

For closer rasterizer consistency, export slides from PowerPoint to a local PDF,
then use the same Poppler DPI on both PDFs:

```sh
mkdir -p compat/references/local/001-basic-shapes
pdftoppm -r 144 -png path/to/powerpoint-export.pdf \
  compat/references/local/001-basic-shapes/Slide
pdfinfo path/to/powerpoint-export.pdf
```

Record `Microsoft PowerPoint (PDF export; Poppler rasterization)` as the renderer
and both tool versions in the notes. PowerPoint PDF export and direct PNG export
are distinct reference pathways; compare like with like across a benchmark run.

If PowerPoint is unavailable, open/export using LibreOffice and record
`LibreOffice`, its actual version and settings. Its layout and font handling can
differ from PowerPoint. A match to LibreOffice cannot establish a match to
PowerPoint. Never use o2p output as an independent fidelity reference; self
comparison is useful only for testing the harness.

## Single-case and private comparison

```sh
compat/.venv/bin/python compat/scripts/compat.py check \
  compat/corpus/public/001-basic-shapes.pptx \
  --reference-dir compat/references/local/001-basic-shapes \
  --font-dir fixtures/fonts

compat/.venv/bin/python compat/scripts/compat.py check \
  compat/corpus/private/private-deck.pptx \
  --reference-dir compat/references/private/private-deck \
  --font-dir path/to/locally-licensed-fonts
```

Options include repeated `--font`, `--font-dir`, `--fallback-family`, `--dpi`
(36–300, default 144), `--timeout` (seconds per external command, default 120),
`--adapter` and `--output-dir`. Each case directory has an input-path hash suffix
to avoid collisions. Each run creates a fresh `run-*` artifact directory; old
files cannot contaminate the current PNG count. `result.json` and `report.md`
describe the most recent run; old artifact directories are retained for review.

The result records input path/hash, source checkout commit/tag/dirty state,
engine version, adapter path/binary hash, Poppler version, metric settings,
reference provenance, PDF page count and per-page point dimensions, raster pixel
dimensions, warnings/raw groups, conversion and total duration, per-slide metrics,
overall mean/worst-slide metrics, ratings and diagnostic errors. Source checkout
identity is not a claim that a custom `--adapter` binary was built from it; the
binary hash identifies the executable actually used. Conversion duration excludes
build, rasterization and image comparison. Point sizes are independently read
from the output PDF with `pdfinfo`.

Warnings group by kind, feature and location. Raw messages remain in JSON.
Font/layout and PDF warnings retain zero-based page/textbox/operation locations;
the Markdown slide table is one-based. Warning presence does not silently change
visual ratings: inspect both. A visual PASS may still have unsupported semantics.

## Batch mode and fidelity gaps

```sh
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public \
  --reference-root compat/references/local --font-dir fixtures/fonts

compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/private \
  --reference-root compat/references/private \
  --output-dir compat/outputs/private --report-dir compat/reports/private \
  --font-dir path/to/locally-licensed-fonts
```

Batch discovers `.pptx` cases recursively in deterministic path order. Reference
directories mirror corpus paths without `.pptx`, e.g. `team/deck.pptx` maps to
`<reference-root>/team/deck/reference.json`. Cases continue after an individual
failure. An empty corpus is an error. Batch writes counts for all ratings, a case
table, and JSON/Markdown fidelity-gap tables ranked by warning occurrences.
The gap table also records affected-case counts, so repeated processing of a
shared theme does not masquerade as many distinct unsupported objects. Detailed
locations remain in individual case reports. Gap feature names are retained
verbatim from the existing warnings; no guessed taxonomy hides distinctions.

No-reference batch mode still produces PDFs, PNGs, warnings and gap reports. It
prints, for example, `6 NOT_COMPARED`; it never invents independent references.
Exit status is 0 if every case is PASS or MINOR_DIFFERENCE, 1 if any is major,
failed or unassessed, and 2 for an invalid command/setup before case evaluation.

## Metric and rating interpretation

The metric is a documented variant of the
[SSIM structural comparison model](https://eceweb.uwaterloo.ca/~z70wang/publications/ssim.html),
not the canonical Gaussian-window implementation and not multiscale SSIM.

1. Composite image alpha on white. Retain original pixel dimensions.
2. Compare physical page dimensions within 0.1 point. Record exact page-count
   match, exact pixel-dimension match and aspect-ratio agreement separately.
3. Downsample to at most 640 pixels wide, preserving the actual raster's aspect
   ratio, then apply a 0.6-pixel Gaussian blur. DPI differences can be resampled;
   aspect-ratio disagreement above 0.002 or physical-size mismatch forces major.
4. Calculate luminance SSIM over non-overlapping 8 × 8 blocks, with population
   moments, `C1=(0.01×255)²`, `C2=(0.03×255)²`; edge blocks use available pixels.
   Convert SSIM to bounded difference `(1−SSIM)/2`.
5. Also calculate normalized RGB absolute error in each blurred block. Use the
   larger of structural and color differences per block. Report luminance SSIM
   difference, RGB error, pixel-weighted mean difference and worst-block error.
   The local maximum helps expose omitted small content amid a large blank slide.

Lower difference is better, but a score is not a compatibility percentage. Small
antialiasing changes are softened; geometry shifts, missing content and color
changes remain visible. Raw native-size difference PNGs aid diagnosis and are
not the perceptual metric itself. Images and reference paths are recorded so a
human can inspect both renders side-by-side.

Provisional thresholds for `block-ssim-rgb-v1`:

| Rating | Rule |
| --- | --- |
| PASS | mean difference ≤ 0.015 and worst block ≤ 0.15 |
| MINOR_DIFFERENCE | mean ≤ 0.08 and worst block ≤ 0.60 |
| MAJOR_DIFFERENCE | above either minor threshold, count/physical-size/aspect mismatch |
| FAILED_TO_RENDER | conversion, PDF inspection or rasterization failed |
| NOT_COMPARED | no usable independent reference or image comparison could not complete |

The case rating is the worst slide rating, with count mismatch forcing major.
The overall raw difference is the arithmetic mean of paired-slide differences;
it excludes missing pages, so count mismatch remains separate and dominant.
Threshold boundaries are inclusive and tested. They are engineering triage
defaults, not empirically calibrated human judgments; revise them using reviewed
corpus evidence and version the metric when changing its definition. Avoid
mixing scores from different reference pathways, fonts or metric versions.

## Limits and semantic correctness

A pixel match cannot establish correct Unicode extraction, reading order,
hyperlinks, accessibility, selectable text, hidden slides, structure, or warning
completeness. White-on-white omitted text may be visually invisible. Averaging
can hide small defects; worst-block checks help but can also overreact to local
export/font differences. Downsampling can lose fine details. Review original
renders at full resolution before diagnosing a feature gap.

The harness does not align images or estimate a translation: displacement is a
real fidelity difference. It does not implement gradients, groups, connectors,
arrows, charts, SmartArt, animations, new typography, DOCX, XLSX or Flutter.
PDF/PNG tooling has its own decoder limits and version-dependent behavior; run
untrusted real-world evaluation in an appropriate development environment. Timeouts
bound external command duration, not total heap usage. Pillow's decoder protections
remain enabled. A huge malformed input can fail evaluation without altering the
native library's security policy. References are not bundled; independent Office
fidelity remains unassessed until a developer supplies them.

## Validation

```sh
cargo build --offline -p o2p-cli --bin o2p --example compat_render
compat/.venv/bin/python -m unittest discover -s compat/scripts -v
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public --font-dir fixtures/fonts
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

Tooling tests cover JSON/Markdown reporting, warning grouping, count/size mismatch,
missing references/rasterizer, conversion failure, metric behavior, threshold
boundaries, stale artifacts, provenance validation, batch gaps, deterministic
corpus generation and parity with `o2p convert`. Harness self-comparison tests are
explicitly test-only, never distributed as PowerPoint references.

## Fidelity 2A measurement against independent PowerPoint references

Fidelity 2A uses clean committed `d1a27f6` as the code baseline and the existing
Microsoft PowerPoint for Mac 16.113.3 (26092714) exports. Reference files, corpus
inputs, metric thresholds, DPI, and configured fixture fonts are unchanged.
[The before/after report](../compat/reports/FIDELITY_2A.md) records all six cases,
each slide's difference, overall means, warning counts, and remaining gaps.

For 001-basic-shapes, slide 1 stays 0.239340, slide 2 stays 0.168438, and overall
stays 0.203889 (MAJOR_DIFFERENCE). Warnings fall 17 -> 3. Across six cases,
image metrics stay unchanged and all remain MAJOR_DIFFERENCE; warning counts
fall 93 -> 20, text fallback 50 -> 1, and clrMap/clrMapOvr 24 -> 0.
No visual success is inferred from warning disappearance. None of these inputs
contains master txStyles; scheme usages do not exercise changed color maps.
Focused synthetic fixtures prove the implemented semantic paths, while broader
PowerPoint-authored inputs are needed to measure their visual impact.

The remaining text fallback is a body placeholder in 004-theme-layout with no
resolved text color. Remaining notesSz, sldMasterIdLst, sldLayoutIdLst and legacy
unwrapped-overflow diagnostics are unchanged. Batch exit 1 is expected while
cases remain MAJOR_DIFFERENCE; it does not mean conversion or tooling failed.

## Fidelity 2B: source identity and rounded geometry/shadows

The six `compat/corpus/public` PPTX files are generated engine exercises. The
PowerPoint PPTX files in `compat/references/local` are different presentations:
matching filenames do not establish matching input identity. Generated public
inputs have no rounded rectangles or outer shadows; local reference inputs
contain the visible rounded cards/buttons and theme shadows. The requested
public batch is therefore retained as a historical comparison, not a paired
visual-fidelity benchmark for these effects. It remains unchanged (all six
MAJOR_DIFFERENCE, twenty warnings).

Fidelity 2B also compares the actual local reference PPTX inputs against their
existing PowerPoint PNGs, using a pre-edit baseline adapter and the current
adapter with identical fonts, DPI, references and thresholds. All twelve paired
slides improve, though all six cases still rate MAJOR_DIFFERENCE. The full
before/after tables, warnings, source hashes and visual review are in
[FIDELITY_2B.md](../compat/reports/FIDELITY_2B.md). No corpus input, reference or
metric threshold was modified. The supplemental command is:

```sh
compat/.venv/bin/python compat/scripts/compat.py batch compat/references/local \
  --reference-root compat/references/local --font-dir fixtures/fonts \
  --output-dir target/fidelity2b/paired-after-images \
  --report-dir target/fidelity2b/paired-after
```

## Fidelity 2C paired public baseline

The current public PPTX inputs are byte-identical to the source.pptx files that
produced the correctly paired local PowerPoint references. This supersedes the
historical mismatch described in the Fidelity 2B section above. The current
corpus does not reproduce from the historical synthetic generator. Tooling tests
check that generator's determinism in temporary directories and separately
validate the current public XML/page count, without replacing benchmark inputs.
Do not regenerate the paired public directory with the historical generator.

Fidelity 2C preserves baseline commit 2b73b7b, input/reference SHA-256 digests,
fonts, 144-DPI rasterization and block-ssim-rgb-v1 thresholds. Every slide improves
its overall difference; localized worst blocks can increase. All six ratings
remain MAJOR_DIFFERENCE. Warning reduction is documented separately from pixels.
004 slide 2's difference improves 20.2%, from 0.128258 to 0.102387. Bullets,
indentation, paragraph gaps and title vertical position improve; substituted
font widths still produce displaced outlines and different wrap points.

[FIDELITY_2C.md](../compat/reports/FIDELITY_2C.md) records per-slide difference and
worst block, per-case overall/rating, total and typography warnings, native image
inspection, exact commands and local artifacts. [FIDELITY_2C.json](../compat/reports/FIDELITY_2C.json)
retains full-precision comparative measurements. Autofit remains deferred;
[TYPOGRAPHY.md](TYPOGRAPHY.md) describes implemented semantics and corpus usage.

## Scalable fonts with auxiliary bitmap strikes

PowerPoint for Mac's Calibri.ttf contains usable TrueType glyf/loca outlines
alongside EBDT/EBLC bitmap strikes. Font registration now accepts those outlines
under the existing validation/permissions/resource checks; bitmap-only fonts and
bitmap rendering remain unsupported. This is a generic table-classification fix,
with no Office-specific discovery, substitution or layout heuristic.

[FONT_COMPATIBILITY.md](../compat/reports/FONT_COMPATIBILITY.md) records local
validation of all four Calibri faces and the correctly paired 001-basic-shapes
comparison using only explicit Calibri regular/bold and Aptos regular/bold.
Overall difference improves from 0.0358493 to 0.0191826, worst block from
0.8319985 to 0.4580673, and the case rating from MAJOR_DIFFERENCE to
MINOR_DIFFERENCE. Fixture-font metrics reproduce the Fidelity 2C baseline exactly;
inputs, references, DPI and thresholds remain unchanged.


## Fidelity 2D — Exact font-family identity and alias-aware resolution

The development adapter and per-case result JSON now include `font_resolutions`,
a deduplicated first-use list of requested family/weight/italic and resolved
canonical family/subfamily/weight/italic, full name, and selected PostScript name.
These are diagnostics, not warnings or rating inputs. Font bytes, paths, and
source text are not included in the entries. Existing adapters without this field
remain supported and yield an empty list. `Substitution` warnings still identify
requested and selected families; successful exact matches remain quiet.

[The Fidelity 2D report](../compat/reports/FIDELITY_2D.md) uses clean commit
11bd2e4, a preserved baseline adapter, and the same explicitly registered local
Office faces before/after. Aptos Display now embeds alongside normal Aptos in
004-theme-layout. Slide 2 difference improves 0.054861 -> 0.047403 and worst block
0.892814 -> 0.755560. Title horizontal bounds almost match the reference, while
a seven-pixel vertical displacement remains at native resolution. Case 004
remains MAJOR_DIFFERENCE; the other five cases retain their exact baseline
metrics and MINOR_DIFFERENCE ratings. The 6/6 MINOR target is not met.

No reference/input image, threshold, DPI, kerning, autofit, paragraph behavior,
or effect changes are included. [FONTS.md](FONTS.md) documents canonical identity,
recognized aliases, deterministic ties, related families, and fallback.
