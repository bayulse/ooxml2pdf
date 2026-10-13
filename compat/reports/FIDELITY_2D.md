# Fidelity 2D — Exact font-family identity and alias-aware resolution

Baseline: clean committed `11bd2e4`. No dependency, proprietary font asset,
input/reference, DPI, threshold, kerning, autofit, or paragraph-layout change.

**Exact-family resolution is fixed. The 6/6 MINOR_DIFFERENCE target is not met:**
five cases remain MINOR_DIFFERENCE and 004-theme-layout remains MAJOR_DIFFERENCE.
The remaining title mismatch is vertical, after horizontal font identity improves.

## Diagnosis and naming model

The old adapter selected the first decodable name ID 16, otherwise ID 1, and
retained only that family plus one ID 6 PostScript name. It dropped other family
and localized records, full names, and subfamilies. Registration appended faces
to a bounded vector; there was no separate family/style bucket or metadata alias
index. Lookup scanned for lowercase family equality, ranked italic then weight,
and broke ties by registration index. Configured aliases were only caller-provided
substitution lists, separate from metadata.

Local Aptos Display records have ID 1 = Aptos Display, ID 16 = Aptos,
ID 17 = Display, and ID 6 = AptosDisplay. The typographic grouping caused both
Aptos and Aptos Display to be stored as Aptos. An Aptos Display request matched
neither stored family, then fell back to Aptos, where normal Aptos won the
registration-order tie. Adding the Display files could not change that result.
The PPTX inheritance/theme path already supplies Aptos Display at 44pt; it is
unchanged. The new telemetry records its exact selection.

Canonical rendering identity is now WWS family (21), otherwise legacy family
(1), otherwise typographic family (16). This intentionally distinguishes a
rendering family from an extended typographic grouping. All decoded names from
1/16/21 remain aliases; records 1/2/4/6/16/17/21/22 retain localization IDs.
Full and PostScript names describe faces rather than participate in family matching.
OS/2 weight/italic still selects style. [FONTS.md](../../docs/FONTS.md) describes
the OpenType rationale, supported encodings, and deterministic spelling policy.

Canonical equality outranks alias equality before italic/nearest-weight rank.
Equal rank compares lowercase canonical family, original canonical family,
PostScript name, then complete bytes. Exact duplicates are equivalent. No prefix
relationship is inferred. Coverage fallback, configured alias/fallback order,
structured substitution warnings, directory sorting/symlink policy, and all
font/resource limits remain. No fonts crate contains PPTX or vendor concepts.

## Local configuration and independent embedding evidence

Both adapters use the same sixteen explicitly registered local Office files,
in filename order, through repeated `--font` flags. Directory registration
continues to skip symlinks, so the existing local links are resolved explicitly.
The configuration includes five Aptos faces (regular/bold/italic/bold-italic/
semibold), three Aptos Display faces (regular/bold/bold-italic), four Arial faces,
and four Calibri faces. File basenames and hashes are recorded in
[FIDELITY_2D.json](FIDELITY_2D.json); no local absolute font paths or font contents
are added to source/tests/reports. Existing local links are unchanged.

`pdffonts` on the baseline 004 PDF lists Aptos, Aptos-Bold, and ArialMT among
embedded faces, with no AptosDisplay. After the fix it lists **AptosDisplay,
Aptos, Aptos-Bold, and ArialMT**, all embedded. This deck requests Display regular,
so a Display bold face is not expected merely because it is registered.

An additional local five-line PDF, with both families registered, independently
embeds Aptos, Aptos-Bold, AptosDisplay, AptosDisplay-Bold, and
AptosDisplay-BoldItalic. Every requested family/weight/italic matches metadata,
shaping succeeds, and exact selections have no warnings. The PDFs and pdffonts
logs remain in ignored `target/fidelity2d`; no Microsoft fonts are copied.

## All six paired cases

Existing PowerPoint references, 144-DPI Poppler rasterization, and
block-ssim-rgb-v1 thresholds are unchanged. All input, fixture and reference files
remain byte-identical. Full per-slide precision, worst blocks, ratings,
configuration and integrity audit are in [FIDELITY_2D.json](FIDELITY_2D.json).

| Case | Overall before | Overall after | Rating before → after |
| --- | ---: | ---: | --- |
| 001-basic-shapes | 0.019182610410 | 0.019182610410 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 002-images | 0.015889994521 | 0.015889994521 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 003-text | 0.024041569263 | 0.024041569263 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 004-theme-layout | 0.033928423961 | 0.030199395815 | MAJOR_DIFFERENCE → MAJOR_DIFFERENCE |
| 005-transforms | 0.014330365216 | 0.014330365216 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |
| 006-style-matrix | 0.012036478980 | 0.012036478980 | MINOR_DIFFERENCE → MINOR_DIFFERENCE |

The five unchanged cases reproduce the supplied fair baseline exactly.
The local 004 before run is 0.033928423961, rather than the supplied approximate
0.033703. Its slide-2 before values are 0.054860790079 / 0.892813926588 rather
than 0.054409 / 0.869960. These differences are reported explicitly; the preserved
adapter measurement is the controlled before value for this configuration.

| 004 measurement | Preserved baseline | After |
| --- | ---: | ---: |
| Slide 1 difference | 0.012996057842424 | 0.012996057842424 |
| Slide 1 worst block | 0.470726988322854 | 0.470726988322854 |
| Slide 2 difference | 0.054860790079091 | 0.047402733788301 |
| Slide 2 worst block | 0.892813926587937 | 0.755559818049309 |

Slide-2 mean decreases 13.59%.
Worst block decreases 15.37%,
but still exceeds the unchanged MINOR threshold of 0.60.

## Native-resolution title inspection

Both difference images and the current/reference slides were inspected at
their original 1920 × 1080 resolution. The horizontally separated title outlines
move into near overlap after selecting the Display face. The title hotspot
decreases but does not disappear. Body text and swatches are unchanged.

Dark-ink bounds within the title region (x/y pixels, right/bottom exclusive):

| Image | Left | Top | Right | Bottom |
| --- | ---: | ---: | ---: | ---: |
| PowerPoint reference | 185 | 103 | 1257 | 176 |
| Before | 147 | 110 | 1295 | 183 |
| After | 184 | 110 | 1258 | 183 |

The new width is 1074px versus 1072px reference, compared with 1148px before.
Vertical displacement remains **7px**. Body baseline differences also remain.
This is evidence of remaining layout/typography mismatch; it does not prove
that kern, autofit, or any single deferred behavior is responsible. No such
feature is implemented here. Correct family identity is established, and work
stops at the requested scope.

[Before slide](../../target/fidelity2d/before/004-theme-layout-c644edfb/run-e3v2kyp_/slide-2.png)

[Before difference](../../target/fidelity2d/before/004-theme-layout-c644edfb/run-e3v2kyp_/difference-2.png)

[After slide](../../target/fidelity2d/after/004-theme-layout-c644edfb/run-5tw7osid/slide-2.png)

[After difference](../../target/fidelity2d/after/004-theme-layout-c644edfb/run-5tw7osid/difference-2.png)


## Observability and regressions

`FontSelection::diagnostic` provides requested/resolved family, weight, italic,
resolved subfamily, full name, and selected PostScript face. The compatibility
adapter collects distinct first-use selections through a resolver wrapper;
`font_resolutions` is retained in telemetry and result JSON without affecting
warnings or ratings. Old adapters without that field remain supported.
Substitution warnings retain requested and selected fallback families.

Nine new synthetic metadata/name-table tests reuse OFL O2P outlines. They cover
overlapping canonical families, four styles, reversed registration, aliases,
related-name/full-name/PostScript separation, localized records, WWS and
typographic fallback, invalid/oversized names, deterministic ties including
identical PostScript names, and grapheme-safe Unicode fallback. Existing O2P
Unicode/embedding/extraction, bitmap-strike acceptance, malformed outlines,
permissions and directory/resource-limit tests pass.

## Validation and reproduction

- cargo fmt: pass.
- cargo clippy --workspace --all-targets --all-features: pass.
- cargo test --workspace: 355 tests pass.
- Compatibility tooling: 27 tests pass, including telemetry preservation,
  legacy adapter support and byte-for-byte conversion parity.
- All six paired conversions/comparisons complete; five MINOR and one MAJOR.
- Input/reference/fixture integrity audit passes with no file-set changes.
- Cargo.toml, Cargo.lock, thresholds, and pre-existing font assets unchanged.

Cargo commands used the populated offline cache. The comparison driver and
preserved adapter are in ignored `target/fidelity2d/run.py` and
`target/fidelity2d/baseline-render`. Reproduce on the same local installation:

```sh
cargo fmt
cargo clippy --offline --workspace --all-targets --all-features
cargo test --offline --workspace
cargo build --offline -p o2p-cli --bin o2p --example compat_render
compat/.venv/bin/python -m unittest discover -s compat/scripts -p 'test_*.py'
compat/.venv/bin/python target/fidelity2d/run.py before
compat/.venv/bin/python target/fidelity2d/run.py after
```

For another installation, supply the same named faces by explicit `--font`
flags; use the documented generic compat.py commands. The local helper and
Office installation are validation inputs, not a portable runtime dependency.
