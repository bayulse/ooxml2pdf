"""Test-only font fixture preparation, using FontTools 4.60.1.

Usage: python generate.py /directory/containing/o2p-Noto*.ttf
Source download URLs/checksums are documented in README.md. No converter uses
this script, FontTools, font subsetting, or variable-font instantiation at runtime.
"""
from pathlib import Path
import hashlib
import sys
from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

destination = Path(__file__).parent
source = Path(sys.argv[1])
latin = list(range(0x20, 0x100)) + list(range(0x300, 0x370)) + list(range(0x370, 0x530)) + [0x2014, 0x2013, 0x2022, 0x20AC]
specs = [
    ("NotoSans-Regular", "O2PTestSans-Regular", "O2P Test Sans", "Regular", latin),
    ("NotoSans-BoldItalic", "O2PTestSans-BoldItalic", "O2P Test Sans", "Bold Italic", latin),
    ("NotoNaskhArabic-Regular", "O2PTestArabic-Regular", "O2P Test Arabic", "Regular", list(range(0x600, 0x700)) + [0x20]),
    ("NotoSansJP", "O2PTestJapanese-Regular", "O2P Test Japanese", "Regular", [ord(c) for c in " 日本語"]),
]
for original, basename, family, style, unicodes in specs:
    path = source / f"o2p-{original}.ttf"
    print(path.name, hashlib.sha256(path.read_bytes()).hexdigest())
    font = TTFont(path, recalcTimestamp=False)
    if "fvar" in font:
        font = instantiateVariableFont(font, {"wght": 400}, inplace=True)
    options = subset.Options()
    options.recalc_timestamp = False
    options.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14, 16, 17]
    options.name_languages = [0x409]
    options.name_legacy = True
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=unicodes)
    subsetter.subset(font)
    # Rename modified OFL fonts; retain original copyright/license name records.
    names = {1: family, 2: style, 3: basename, 4: f"{family} {style}", 6: basename, 16: family, 17: style}
    for name_id, value in names.items():
        font["name"].setName(value, name_id, 3, 1, 0x409)
    font.save(destination / f"{basename}.ttf")
