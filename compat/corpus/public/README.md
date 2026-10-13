# Original public corpus

These six two-slide widescreen decks are created specifically for this project.
They contain original text, shapes, layouts, theme definitions and a generated
color-study PNG. No Office exports, organizational material, personal data or
third-party artwork is included. The corpus and generator are original project
material; see [font fixture licenses](../../../fixtures/fonts/README.md) separately.

| Case | Composition |
| --- | --- |
| 001-basic-shapes | Project dashboard: cards, ellipse, backgrounds and footer |
| 002-images | Original raster artwork, caption panel and surrounding text |
| 003-text | Wrapped brief and centered paragraph summary |
| 004-theme-layout | Theme cards, shared decorations and inherited placeholder geometry |
| 005-transforms | Rotated card and picture, mirrored text-bearing ellipse |
| 006-style-matrix | Matrix fill/line styles and direct fill override |

Each deck has a second review slide. Slide size is 960 × 540 points (13⅓ × 7½
inches). Use the explicit `fixtures/fonts` directory for shaped text conversion.
Install the O2P test fonts locally if matching them in a reference renderer.

Regenerate deterministically:

```sh
python3 compat/scripts/generate_corpus.py
```

ZIP timestamps and ordering are fixed. Existing parser fixtures in `fixtures/pptx`
remain untouched. These cases exercise existing behavior; they are not a claim of
Office validation. Open/export them in PowerPoint and inspect any repair prompts
before accepting reference images.
