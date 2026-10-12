# Redistributable test fonts

These are **test-only modified fonts**, licensed under SIL Open Font License 1.1.
They are not bundled into the converter or selected implicitly at runtime.
Original copyright/license name records remain in every font. Modified family
and PostScript names use `O2P Test …`, avoiding original/reserved font names.
No Microsoft fonts are included.

| Fixture | Upstream source | Coverage retained | Approximate size |
| --- | --- | --- | --- |
| O2PTestSans-Regular.ttf | Noto Sans Regular | ASCII, Latin-1, combining marks, Greek, Cyrillic, common punctuation; layout closure/ligatures | 137 KiB |
| O2PTestSans-BoldItalic.ttf | Noto Sans Bold Italic | Same; real weight/style matching | 136 KiB |
| O2PTestArabic-Regular.ttf | Noto Naskh Arabic Regular | Arabic block, space, contextual shaping closure | 87 KiB |
| O2PTestJapanese-Regular.ttf | Noto Sans JP | Space and 日本語, static regular instance | 3 KiB |

Total font assets: approximately 361 KiB. The Japanese fixture demonstrates CJK
fallback and embedding without committing a multi-megabyte CJK font. It is not
a Japanese coverage font for production. Consumers can register a full static
TrueType CJK font through the same API.

Copyright notices from the originals:

- Sans Regular: Copyright 2015–2021 Google LLC. All Rights Reserved.
- Sans Bold Italic: Copyright 2015–2022 Google LLC. All Rights Reserved.
- Arabic: Copyright 2019–2021 Google LLC. All Rights Reserved.
- Japanese: (c) 2014–2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source'.

See [OFL-Noto.txt](OFL-Noto.txt) and [OFL-NotoSansJP.txt](OFL-NotoSansJP.txt).
The font fixtures remain under OFL, independently of the Rust source license.

## Reproduction

Use Python with FontTools **4.60.1** for fixture preparation only. Download these
sources into a temporary directory under the names below, verify SHA-256, then
run `python fixtures/fonts/generate.py <source-directory>`. The script performs
test-asset subsetting and static instantiation before renaming the fonts. These
operations are not implemented or invoked by the Rust runtime.

| Download name | Source | SHA-256 |
| --- | --- | --- |
| o2p-NotoSans-Regular.ttf | [noto-fonts/hinted/ttf/NotoSans/NotoSans-Regular.ttf](https://github.com/notofonts/noto-fonts/blob/main/hinted/ttf/NotoSans/NotoSans-Regular.ttf) | b85c38ecea8a7cfb39c24e395a4007474fa5a4fc864f6ee33309eb4948d232d5 |
| o2p-NotoSans-BoldItalic.ttf | [noto-fonts/hinted/ttf/NotoSans/NotoSans-BoldItalic.ttf](https://github.com/notofonts/noto-fonts/blob/main/hinted/ttf/NotoSans/NotoSans-BoldItalic.ttf) | 6edf4227ef0fa846aca70e86a307804ca4401741830f5b3af0f2554abe2b8466 |
| o2p-NotoNaskhArabic-Regular.ttf | [noto-fonts/hinted/ttf/NotoNaskhArabic/NotoNaskhArabic-Regular.ttf](https://github.com/notofonts/noto-fonts/blob/main/hinted/ttf/NotoNaskhArabic/NotoNaskhArabic-Regular.ttf) | 2f4b88e6ee50fa82c617e2d1d4ba18281cb1c6cd71c3af3ec64970c23995db4b |
| o2p-NotoSansJP.ttf | [google/fonts/ofl/notosansjp/NotoSansJP\[wght\].ttf](https://github.com/google/fonts/blob/main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf) | c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f |

Upstream main branches can change: the checksums identify the exact inputs.
Source downloads (including the ~9 MiB variable JP source) are not committed.
The committed Japanese output has no variation tables. Regeneration preserves
source timestamps for reproducibility. FontTools and network access are optional
developer tools, never converter dependencies.
