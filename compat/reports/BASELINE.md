# Initial compatibility corpus observations

Rendering baseline: `v0.1.0-alpha.4` / `82e9e8a`; engine version `0.1.0`.
Evaluation setup: explicit `fixtures/fonts`, Poppler 26.07.0, PNG at 144 DPI.
These observations were collected with the initial evaluation tooling in the
workspace. Runtime rendering code and dependencies are unchanged from baseline.

All six original public cases produced two 960 × 540 point PDF pages and two
1920 × 1080 pixel slide PNGs. No independent Office references were supplied.
Every visual rating is therefore **NOT_COMPARED**. This is a rendering smoke
check and warning inventory, not a PowerPoint fidelity result.

| Case | Rendered slides | Warning occurrences | Visual rating |
| --- | --- | --- | --- |
| 001-basic-shapes | 2 | 17 | NOT_COMPARED |
| 002-images | 2 | 15 | NOT_COMPARED |
| 003-text | 2 | 17 | NOT_COMPARED |
| 004-theme-layout | 2 | 14 | NOT_COMPARED |
| 005-transforms | 2 | 16 | NOT_COMPARED |
| 006-style-matrix | 2 | 14 | NOT_COMPARED |

The raw warning inventory contains 93 occurrences. The leading groups are:

| Kind | Feature (abbreviated here only) | Occurrences | Affected cases |
| --- | --- | --- | --- |
| UnsupportedSlideContent | text inheritance/font resolution fallback diagnostic | 50 | 6 |
| UnsupportedSlideContent | PresentationML clrMapOvr | 18 | 6 |
| UnsupportedPresentationMetadata | PresentationML notesSz | 6 | 6 |
| UnsupportedPresentationMetadata | PresentationML sldMasterIdLst | 6 | 6 |
| UnsupportedSlideContent | PresentationML clrMap | 6 | 6 |
| UnsupportedSlideContent | PresentationML sldLayoutIdLst | 6 | 6 |
| UnsupportedSlideContent | legacy unwrapped text overflow diagnostic | 1 | 1 |

These counts preserve existing parser diagnostics. Generic text fallback and
legacy overflow diagnostics are not evidence that configured shaped text failed.
Shared-part metadata can be reported while other supported inheritance behavior
works. Review locations and actual references before interpreting these notices
as missing renderer functionality. No gradient, group, chart or SmartArt ranking
can be inferred from this intentionally small foundation corpus.

Next step: export independent PowerPoint references with recorded fonts/settings,
compare these cases, then add representative private decks and review frequent
warnings alongside visible defects. Select renderer work from that evidence.

Regenerate the detailed JSON/Markdown inventory locally:

```sh
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public --font-dir fixtures/fonts
```

This exits 1 while references are absent and writes reports into ignored
`compat/reports/public/` and per-case artifacts into `compat/outputs/public/`.
