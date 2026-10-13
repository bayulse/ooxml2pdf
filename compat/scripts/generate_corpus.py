#!/usr/bin/env python3
"""Original, deterministic composed decks. Standard library only; no Office assets."""
from html import escape
from pathlib import Path
import struct
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
P = "http://schemas.openxmlformats.org/presentationml/2006/main"
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
EMU = 12700


def color(value):
    return f'<a:solidFill><a:srgbClr val="{value}"/></a:solidFill>'


def shape(id, x, y, w, h, fill="FFFFFF", geometry="rect", text=None,
          size=20, ink="23354D", rotation=0, flip="", style="", placeholder="", align="l"):
    transform = f'<a:xfrm rot="{int(rotation * 60000)}" {flip}><a:off x="{x * EMU}" y="{y * EMU}"/><a:ext cx="{w * EMU}" cy="{h * EMU}"/></a:xfrm>'
    paint = color(fill) if fill else ("" if style else "<a:noFill/>")
    outline = '<a:ln w="12700">' + color("718096") + '</a:ln>' if geometry == "line" else ("" if style else '<a:ln><a:noFill/></a:ln>')
    body = ""
    if text is not None:
        paragraphs = "".join(f'<a:p><a:pPr algn="{align}"/><a:r><a:rPr lang="en-US" sz="{size * 100}">{color(ink)}<a:latin typeface="O2P Test Sans"/></a:rPr><a:t>{escape(line)}</a:t></a:r><a:endParaRPr sz="{size * 100}"/></a:p>' for line in text.split("\n"))
        body = f'<p:txBody><a:bodyPr wrap="square" lIns="76200" rIns="76200" tIns="76200" bIns="76200"/><a:lstStyle/>{paragraphs}</p:txBody>'
    return f'<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Case shape {id}"/><p:cNvSpPr/><p:nvPr>{placeholder}</p:nvPr></p:nvSpPr><p:spPr>{transform}<a:prstGeom prst="{geometry}"><a:avLst/></a:prstGeom>{paint}{outline}</p:spPr>{style}{body}</p:sp>'


def tree(shapes):
    return '<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>' + shapes + '</p:spTree>'


def document(kind, content, extra=""):
    return f'<p:{kind} xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}">{content}{extra}</p:{kind}>'


def rels(items):
    return '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' + ''.join(f'<Relationship Id="{id}" Type="{R}/{kind}" Target="{target}"/>' for id, kind, target in items) + '</Relationships>'


def theme():
    palette = dict(dk1="23354D", lt1="FFFFFF", dk2="102A43", lt2="F1F5F9", accent1="2563EB", accent2="0D9488", accent3="F59E0B", accent4="8B5CF6", accent5="EF4444", accent6="64748B", hlink="2563EB", folHlink="8B5CF6")
    colors = ''.join(f'<a:{key}><a:srgbClr val="{value}"/></a:{key}>' for key, value in palette.items())
    fonts = ''.join(f'<a:{role}Font><a:latin typeface="O2P Test Sans"/><a:ea typeface=""/><a:cs typeface=""/></a:{role}Font>' for role in ("major", "minor"))
    fills = '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>' * 3
    lines = ''.join(f'<a:ln w="{width}" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln>' for width in (12700, 25400, 38100))
    return f'<a:theme xmlns:a="{A}" name="O2P Compatibility"><a:themeElements><a:clrScheme name="O2P">{colors}</a:clrScheme><a:fontScheme name="O2P">{fonts}</a:fontScheme><a:fmtScheme name="O2P"><a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>'


def png():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    width, height = 160, 100
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            rows.extend((int(x * 255 / width), int(y * 255 / height), 160 if x < width / 2 else 80))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(rows))) + chunk(b"IEND", b"")


def picture(id, x, y, w, h, rotation=0):
    return f'<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Original color study"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="image"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm rot="{rotation * 60000}"><a:off x="{x * EMU}" y="{y * EMU}"/><a:ext cx="{w * EMU}" cy="{h * EMU}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>'


def chrome(title, subtitle, page):
    return shape(2, 36, 28, 888, 56, fill=None, text=title, size=30) + shape(3, 36, 88, 888, 44, fill=None, text=subtitle, size=16, ink="64748B") + shape(4, 36, 490, 888, 2, "CBD5E1") + shape(5, 36, 500, 888, 32, fill=None, text=f"ooxml2pdf / original compatibility corpus / {page}", size=12)


def cases():
    shapes = ''.join(shape(10 + i, 52 + i * 290, 165, 260, 230, c, text=f"{label}\n{value}", size=24, geometry="ellipse" if i == 1 else "rect") for i, (c, label, value) in enumerate((("DBEAFE", "Plan", "12 tasks"), ("CCFBF1", "Build", "8 complete"), ("FEF3C7", "Review", "4 remaining"))))
    yield "001-basic-shapes", "Project overview", "A dashboard composed from primitive shapes", shapes
    yield "002-images", "Field notes", "Original generated raster artwork with surrounding text", picture(10, 52, 155, 480, 300) + shape(11, 565, 155, 330, 300, "FFFFFF", text="Color study\nAn original raster asset.\nScaled to fit the panel.\nNo external images.", size=22)
    yield "003-text", "Design brief", "Measured wrapping and paragraph alignment", shape(10, 52, 155, 400, 300, "FFFFFF", text="Résumé and café\nClear text connects an idea to its audience. This paragraph deliberately wraps across several measured lines.\nKeep the brief concise.", size=22) + shape(11, 485, 155, 410, 300, "DBEAFE", text="Milestone review\nAlignment matters\nA centered summary", size=24, align="ctr")
    yield "004-theme-layout", "Shared design language", "Theme paint and layout/master decoration", ''.join(shape(10 + i, 52 + i * 290, 170, 260, 225, None, text=f"Theme card {i + 1}\nShared palette", style=f'<p:style><a:lnRef idx="1"><a:schemeClr val="accent{i + 1}"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent{i + 1}"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="dk1"/></a:fontRef></p:style>') for i in range(3))
    yield "005-transforms", "Alternate perspectives", "Rotated and mirrored elements preserve their local geometry", shape(10, 85, 200, 230, 160, "DBEAFE", text="Rotate 15 degrees", rotation=15) + shape(11, 370, 200, 200, 160, "CCFBF1", geometry="ellipse", text="Mirror", flip='flipH="1"') + picture(12, 655, 180, 210, 220, rotation=-15)
    yield "006-style-matrix", "Style hierarchy", "DrawingML fill and line references with direct overrides", ''.join(shape(10 + i, 52 + i * 290, 165, 260, 250, "FEF3C7" if i == 2 else None, text=f"Style {i + 1}\n" + ("Direct fill override" if i == 2 else "Matrix fill and border"), style=f'<p:style><a:lnRef idx="{i + 1}"><a:schemeClr val="accent2"/></a:lnRef><a:fillRef idx="{i + 1}"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="major"><a:schemeClr val="dk1"/></a:fontRef></p:style>') for i in range(3))


def build_deck(name, title, subtitle, content, directory=None):
    parts = {}
    overrides = {"ppt/presentation.xml": "presentation.main", "ppt/slideMasters/slideMaster1.xml": "slideMaster", "ppt/slideLayouts/slideLayout1.xml": "slideLayout", "ppt/slides/slide1.xml": "slide", "ppt/slides/slide2.xml": "slide"}
    types = '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>'
    types += ''.join(f'<Override PartName="/{p}" ContentType="application/vnd.openxmlformats-officedocument.presentationml.{t}+xml"/>' for p, t in overrides.items())
    parts["[Content_Types].xml"] = types + '<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>'
    parts["_rels/.rels"] = rels([("office", "officeDocument", "ppt/presentation.xml")])
    parts["ppt/presentation.xml"] = document("presentation", '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="master"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id="256" r:id="slide1"/><p:sldId id="257" r:id="slide2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/>')
    parts["ppt/_rels/presentation.xml.rels"] = rels([("master", "slideMaster", "slideMasters/slideMaster1.xml"), ("theme", "theme", "theme/theme1.xml"), ("slide1", "slide", "slides/slide1.xml"), ("slide2", "slide", "slides/slide2.xml")])
    cmap = '<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>'
    master_placeholder = shape(80, 52, 420, 840, 60, None, placeholder='<p:ph type="body" idx="1"/>')
    master = '<p:cSld><p:bg><p:bgPr>' + color("F1F5F9") + '</p:bgPr></p:bg>' + tree(shape(90, 0, 0, 960, 8, "2563EB") + master_placeholder) + '</p:cSld>'
    parts["ppt/slideMasters/slideMaster1.xml"] = document("sldMaster", master + cmap + '<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="layout"/></p:sldLayoutIdLst>')
    parts["ppt/slideMasters/_rels/slideMaster1.xml.rels"] = rels([("layout", "slideLayout", "../slideLayouts/slideLayout1.xml"), ("theme", "theme", "../theme/theme1.xml")])
    # Sparse placeholder is composed into a real slide; no duplicate prompt text.
    ph = shape(80, 52, 420, 840, 60, None, placeholder='<p:ph type="body" idx="1"/>')
    parts["ppt/slideLayouts/slideLayout1.xml"] = document("sldLayout", '<p:cSld name="O2P Blank">' + tree(ph + shape(91, 930, 28, 8, 440, "CCFBF1")) + '</p:cSld>', '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>')
    parts["ppt/slideLayouts/_rels/slideLayout1.xml.rels"] = rels([("master", "slideMaster", "../slideMasters/slideMaster1.xml")])
    parts["ppt/theme/theme1.xml"] = theme()
    parts["ppt/media/art.png"] = png()
    for page in (1, 2):
        body = content if page == 1 else shape(10, 52, 155, 840, 245, "FFFFFF", text=f"{title}: review\nInspect the composition, text, palette and placement.\nCompare against a separately rendered reference.", size=26)
        if name == "004-theme-layout" and page == 1:
            body += '<p:sp><p:nvSpPr><p:cNvPr id="81" name="Inherited body"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1800"><a:latin typeface="O2P Test Sans"/></a:rPr><a:t>Geometry inherited from the layout placeholder.</a:t></a:r></a:p></p:txBody></p:sp>'
        parts[f"ppt/slides/slide{page}.xml"] = document("sld", '<p:cSld name="' + escape(title) + '">' + tree(chrome(title, subtitle, page) + body) + '</p:cSld>', '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>')
        parts[f"ppt/slides/_rels/slide{page}.xml.rels"] = rels([("layout", "slideLayout", "../slideLayouts/slideLayout1.xml"), ("image", "image", "../media/art.png")])
    directory = Path(directory) if directory else ROOT / "compat/corpus/public"
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / (name + ".pptx")
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for part, data in sorted(parts.items()):
            info = zipfile.ZipInfo(part, date_time=(2020, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, data.encode() if isinstance(data, str) else data)
    return path


if __name__ == "__main__":
    for case in cases():
        print(build_deck(*case).name)
