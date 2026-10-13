#![allow(dead_code)]

// Reuse the existing deterministic test-only ZIP builder, without changing the
// package reader or its Milestone 1A fixtures.
#[path = "../../../o2p-ooxml/tests/support/mod.rs"]
mod package;

pub use package::{OFFICE_RELS, minimal, relation, rels, zip};

pub const PML: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub const STANDARD_SIZE: &str = "<p:sldSz cx='9144000' cy='6858000' type='screen4x3'/>";
pub const WIDE_SIZE: &str = "<p:sldSz cx='12192000' cy='6858000' type='screen16x9'/>";

pub type Parts = Vec<(String, Vec<u8>)>;

/// Old primitive-only fixtures intentionally have no layout; all other warnings
/// must still be checked by their original rendering regression tests.
pub fn is_missing_layout_warning(warning: &o2p_pptx::PptxWarning) -> bool {
    matches!(warning, o2p_pptx::PptxWarning::UnsupportedSlideContent { name, .. } if name == "missing slideLayout relationship: inheritance unavailable")
}

pub fn presentation(inner: &str) -> Vec<u8> {
    format!("<p:presentation xmlns:p='{PML}' xmlns:r='{OFFICE_RELS}'>{inner}</p:presentation>")
        .into_bytes()
}

pub fn ids(order: &[usize]) -> String {
    let mut list = "<p:sldIdLst>".to_owned();
    for (index, relationship) in order.iter().enumerate() {
        list.push_str(&format!(
            "<p:sldId id='{}' r:id='rId{relationship}'/>",
            256 + index
        ));
    }
    list.push_str("</p:sldIdLst>");
    list
}

pub fn replace(parts: &mut [(String, Vec<u8>)], name: &str, bytes: Vec<u8>) {
    parts.iter_mut().find(|(part, _)| part == name).unwrap().1 = bytes;
}

pub fn deck(slides: usize, size: &str) -> Parts {
    let mut parts = minimal(slides, false);
    replace(
        &mut parts,
        "ppt/presentation.xml",
        presentation(&format!("{}{size}", ids(&(1..=slides).collect::<Vec<_>>()))),
    );
    parts
}

pub fn reordered() -> Parts {
    let mut parts = deck(3, STANDARD_SIZE);
    replace(
        &mut parts,
        "ppt/presentation.xml",
        presentation(&format!(
            "<p:sldIdLst><p:sldId id='900' r:id='rId2'/><p:sldId id='300' r:id='rId3'/><p:sldId id='600' r:id='rId1'/></p:sldIdLst>{STANDARD_SIZE}"
        )),
    );
    replace(
        &mut parts,
        "ppt/_rels/presentation.xml.rels",
        rels(&format!(
            "{}{}{}",
            relation("rId1", "slide", "slides/slide2.xml"),
            relation("rId3", "slide", "slides/slide1.xml"),
            relation("rId2", "slide", "slides/slide3.xml"),
        )),
    );
    parts
}

pub fn fixtures() -> Vec<(&'static str, Parts)> {
    vec![
        ("one-slide-standard.pptx", deck(1, STANDARD_SIZE)),
        ("three-slides-standard.pptx", deck(3, STANDARD_SIZE)),
        ("three-slides-16x9.pptx", deck(3, WIDE_SIZE)),
        ("reordered-slides.pptx", reordered()),
        ("missing-slide-size.pptx", deck(1, "")),
        (
            "malformed-slide-size.pptx",
            deck(1, "<p:sldSz cx='0' cy='6858000'/>"),
        ),
        (
            "deep-presentation.pptx",
            deck(
                1,
                &format!(
                    "{STANDARD_SIZE}<p:extLst>{}{}</p:extLst>",
                    "<p:ext>".repeat(63),
                    "</p:ext>".repeat(63)
                ),
            ),
        ),
    ]
}

pub const DML: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
#[path = "themes.rs"]
mod themes;
#[allow(unused_imports)]
pub use themes::*;
mod fidelity2a;
mod fidelity2b;
mod fidelity2c;
#[allow(unused_imports)]
pub use fidelity2b::*;
#[allow(unused_imports)]
pub use fidelity2c::*;
#[path = "inheritance.rs"]
mod inheritance;
mod styles;
#[allow(unused_imports)]
pub use fidelity2a::*;
#[allow(unused_imports)]
pub use inheritance::*;
#[allow(unused_imports)]
pub use styles::*;
pub const NONTRIVIAL_RECT: (i64, i64, i64, i64) = (1_234_567, 765_432, 3_456_789, 2_345_678);

pub fn solid(color: &str) -> String {
    format!("<a:solidFill><a:srgbClr val='{color}'/></a:solidFill>")
}

pub fn rectangle(id: usize, coordinates: (i64, i64, i64, i64), fill: &str, line: &str) -> String {
    let (x, y, cx, cy) = coordinates;
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id='{id}' name='Rectangle {id}'/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x='{x}' y='{y}'/><a:ext cx='{cx}' cy='{cy}'/></a:xfrm><a:prstGeom prst='rect'><a:avLst/></a:prstGeom>{fill}{line}</p:spPr></p:sp>"
    )
}

pub fn slide(background: &str, shapes: &str) -> Vec<u8> {
    format!("<p:sld xmlns:p='{PML}' xmlns:a='{DML}'><p:cSld>{background}<p:spTree><p:nvGrpSpPr><p:cNvPr id='1' name=''/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld></p:sld>").into_bytes()
}

pub fn background(color: &str) -> String {
    format!("<p:bg><p:bgPr>{}</p:bgPr></p:bg>", solid(color))
}

pub fn content_deck(background: &str, shapes: &str) -> Parts {
    let mut parts = deck(1, STANDARD_SIZE);
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        slide(background, shapes),
    );
    parts
}

pub fn content_fixtures() -> Vec<(&'static str, Parts)> {
    let no_line = "<a:ln><a:noFill/></a:ln>";
    let border = format!("<a:ln w='31750'>{}</a:ln>", solid("D02030"));
    let first = rectangle(2, NONTRIVIAL_RECT, &solid("2060D0"), no_line);
    let second = rectangle(
        3,
        (2_540_000, 1_270_000, 2_540_000, 1_270_000),
        &solid("F0B020"),
        &border,
    );
    let third = rectangle(
        4,
        (-127_000, 254_000, 1_270_000, 635_000),
        "<a:noFill/>",
        &border,
    );
    let mut reordered = reordered();
    for (index, color) in ["FF0000", "00FF00", "0000FF"].iter().enumerate() {
        replace(
            &mut reordered,
            &format!("ppt/slides/slide{}.xml", index + 1),
            slide("", &rectangle(2, NONTRIVIAL_RECT, &solid(color), no_line)),
        );
    }
    vec![
        (
            "solid-background.pptx",
            content_deck(&background("304050"), ""),
        ),
        (
            "one-rectangle.pptx",
            content_deck(
                "",
                &rectangle(
                    2,
                    (914_400, 914_400, 1_828_800, 914_400),
                    &solid("000000"),
                    no_line,
                ),
            ),
        ),
        ("filled-rectangle.pptx", content_deck("", &first)),
        (
            "outlined-rectangle.pptx",
            content_deck(
                "",
                &rectangle(2, NONTRIVIAL_RECT, &solid("CCDDEE"), &border),
            ),
        ),
        (
            "no-fill-rectangle.pptx",
            content_deck(
                &background("E0E8F0"),
                &rectangle(2, NONTRIVIAL_RECT, "<a:noFill/>", &border),
            ),
        ),
        (
            "multiple-rectangles.pptx",
            content_deck(&background("FFFFFF"), &format!("{first}{second}{third}")),
        ),
        ("reordered-rectangles.pptx", reordered),
        (
            "unsupported-style.pptx",
            content_deck(
                "",
                &rectangle(
                    2,
                    NONTRIVIAL_RECT,
                    "<a:solidFill><a:schemeClr val='accent1'/></a:solidFill>",
                    &format!(
                        "<a:ln w='12700'>{}<a:prstDash val='dash'/></a:ln>",
                        solid("000000")
                    ),
                ),
            ),
        ),
    ]
}

pub fn content_error_fixtures() -> Vec<(&'static str, Parts)> {
    let mut invalid_root = deck(1, STANDARD_SIZE);
    replace(
        &mut invalid_root,
        "ppt/slides/slide1.xml",
        b"<not-a-slide/>".to_vec(),
    );
    let deep = content_deck(
        "",
        &format!("{}{}", "<p:ext>".repeat(65), "</p:ext>".repeat(65)),
    );
    vec![
        ("invalid-slide-root.pptx", invalid_root),
        ("deep-slide.pptx", deep),
    ]
}

pub fn ellipse(id: usize, coordinates: (i64, i64, i64, i64), fill: &str, line: &str) -> String {
    rectangle(id, coordinates, fill, line)
        .replace("prst='rect'", "prst='ellipse'")
        .replace("Rectangle", "Ellipse")
}

pub fn straight_line(id: usize, coordinates: (i64, i64, i64, i64), line: &str) -> String {
    rectangle(id, coordinates, "", line)
        .replace("prst='rect'", "prst='line'")
        .replace("Rectangle", "Line")
        .replace("p:sp>", "p:cxnSp>")
        .replace("nvSpPr", "nvCxnSpPr")
        .replace("cNvSpPr", "cNvCxnSpPr")
}

pub fn outline(color: &str, width: i64) -> String {
    format!("<a:ln w='{width}'>{}</a:ln>", solid(color))
}

pub fn raster(jpeg: bool) -> Vec<u8> {
    use image::ImageEncoder;
    let mut bytes = vec![];
    if jpeg {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 100)
            .write_image(
                &[20, 160, 60].repeat(6),
                3,
                2,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
    } else {
        // Distinct quadrants prove orientation; opaque alpha exercises RGBA decoding.
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &[
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                ],
                2,
                2,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
    }
    bytes
}

pub fn picture(id: usize, coordinates: (i64, i64, i64, i64)) -> String {
    let (x, y, cx, cy) = coordinates;
    format!(
        "<p:pic><p:nvPicPr><p:cNvPr id='{id}' name='Synthetic image'/><p:cNvPicPr><a:picLocks noChangeAspect='1'/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip xmlns:r='{OFFICE_RELS}' r:embed='rImage'/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x='{x}' y='{y}'/><a:ext cx='{cx}' cy='{cy}'/></a:xfrm><a:prstGeom prst='rect'><a:avLst/></a:prstGeom></p:spPr></p:pic>"
    )
}

pub fn image_deck(jpeg: bool, drawings: &str) -> Parts {
    let mut parts = content_deck(&background("FFFFFF"), drawings);
    let extension = if jpeg { "jpeg" } else { "png" };
    let types = parts
        .iter()
        .find(|(name, _)| name == "[Content_Types].xml")
        .unwrap()
        .1
        .clone();
    replace(
        &mut parts,
        "[Content_Types].xml",
        String::from_utf8(types)
            .unwrap()
            .replace(
                "</Types>",
                "<Default Extension='jpeg' ContentType='image/jpeg'/></Types>",
            )
            .into_bytes(),
    );
    parts.push((format!("ppt/media/synthetic.{extension}"), raster(jpeg)));
    parts.push((
        "ppt/slides/_rels/slide1.xml.rels".into(),
        rels(&relation(
            "rImage",
            "image",
            &format!("../media/synthetic.{extension}"),
        )),
    ));
    parts
}

pub fn image_fixtures() -> Vec<(&'static str, Parts)> {
    let normal = (914_400, 914_400, 1_828_800, 1_828_800);
    let image = picture(3, normal);
    let rect = rectangle(
        2,
        (457_200, 457_200, 1_828_800, 1_828_800),
        &solid("3060D0"),
        "<a:ln><a:noFill/></a:ln>",
    );
    let ellipse = ellipse(
        4,
        (1_828_800, 1_828_800, 1_828_800, 914_400),
        &solid("209060"),
        "<a:ln><a:noFill/></a:ln>",
    );
    let line = straight_line(
        5,
        (914_400, 1_828_800, 3_657_600, 0),
        &outline("D02030", 101_600),
    );
    vec![
        ("one-png.pptx", image_deck(false, &image)),
        (
            "one-jpeg.pptx",
            image_deck(true, &picture(2, (914_400, 914_400, 2_743_200, 1_828_800))),
        ),
        (
            "positioned-image.pptx",
            image_deck(false, &picture(2, NONTRIVIAL_RECT)),
        ),
        (
            "scaled-image.pptx",
            image_deck(false, &picture(2, (914_400, 914_400, 3_657_600, 914_400))),
        ),
        (
            "image-with-shapes.pptx",
            image_deck(false, &format!("{rect}{image}{ellipse}{line}")),
        ),
        (
            "overlapping-image-shape.pptx",
            image_deck(false, &format!("{image}{rect}")),
        ),
    ]
}

pub fn shape_fixtures() -> Vec<(&'static str, Parts)> {
    let no_line = "<a:ln><a:noFill/></a:ln>";
    let ellipse_box = (914_400, 914_400, 2_743_200, 1_828_800);
    let basic = ellipse(2, ellipse_box, &solid("209060"), no_line);
    let filled = ellipse(2, NONTRIVIAL_RECT, &solid("209060"), no_line);
    let outlined = ellipse(2, ellipse_box, &solid("CCDDEE"), &outline("D02030", 31_750));
    let no_fill = ellipse(2, ellipse_box, "<a:noFill/>", &outline("209060", 25_400));
    let line = straight_line(
        2,
        (914_400, 914_400, 3_657_600, 1_828_800),
        &outline("D02030", 12_700),
    );
    let thick = straight_line(
        2,
        (914_400, 914_400, 3_657_600, 0),
        &outline("3060D0", 101_600),
    );
    let rect = rectangle(
        2,
        (914_400, 914_400, 2_743_200, 1_828_800),
        &solid("3060D0"),
        no_line,
    );
    let mixed = format!(
        "{rect}{}{}",
        ellipse(
            3,
            (4_572_000, 914_400, 2_743_200, 1_828_800),
            &solid("209060"),
            &outline("204030", 25_400)
        ),
        straight_line(
            4,
            (914_400, 3_657_600, 6_400_800, 914_400),
            &outline("D02030", 63_500)
        )
    );
    let overlapping = format!(
        "{rect}{}{}",
        ellipse(
            3,
            (1_371_600, 1_371_600, 2_743_200, 1_828_800),
            &solid("209060"),
            no_line
        ),
        straight_line(
            4,
            (914_400, 1_828_800, 3_657_600, 0),
            &outline("D02030", 101_600)
        )
    );
    vec![
        ("one-ellipse.pptx", content_deck("", &basic)),
        ("filled-ellipse.pptx", content_deck("", &filled)),
        ("outlined-ellipse.pptx", content_deck("", &outlined)),
        (
            "no-fill-ellipse.pptx",
            content_deck(&background("CCDDEE"), &no_fill),
        ),
        ("one-line.pptx", content_deck("", &line)),
        ("thick-line.pptx", content_deck("", &thick)),
        ("mixed-shapes.pptx", content_deck("", &mixed)),
        ("overlapping-shapes.pptx", content_deck("", &overlapping)),
    ]
}

pub fn text_run(text: &str, properties: &str) -> String {
    let literal = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<a:r><a:rPr {properties}/><a:t>{literal}</a:t></a:r>")
}

pub fn text_shape(
    id: usize,
    coordinates: (i64, i64, i64, i64),
    paragraphs: &str,
    fill: &str,
    line: &str,
) -> String {
    rectangle(id, coordinates, fill, line).replace("</p:sp>", &format!("<p:txBody><a:bodyPr wrap='none'><a:noAutofit/></a:bodyPr><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"))
}

pub const TEXT_RECT: (i64, i64, i64, i64) = (914_400, 914_400, 4_572_000, 1_828_800);

pub fn text_fixtures() -> Vec<(&'static str, Parts)> {
    let no_line = "<a:ln><a:noFill/></a:ln>";
    let paragraph = |text: &str| format!("<a:p>{}</a:p>", text_run(text, "sz='2400'"));
    let box_deck = |paragraphs: &str| {
        content_deck(
            "",
            &text_shape(2, TEXT_RECT, paragraphs, "<a:noFill/>", no_line),
        )
    };
    let unicode: String = ["Hello", "Indonesia", "Résumé", "日本語"]
        .iter()
        .map(|text| paragraph(text))
        .collect();
    let runs = format!(
        "<a:p>{}{}{}</a:p>",
        text_run("Hello ", "sz='2400'"),
        text_run("Indonesia ", "sz='1800' b='1'"),
        text_run("Résumé", "sz='2400' i='1'")
    );
    let styled = format!(
        "<a:p><a:r><a:rPr sz='2800' b='1' i='1'>{}</a:rPr><a:t>Bold italic red</a:t></a:r><a:br/>{}</a:p>",
        solid("D02030"),
        text_run("Regular text", "sz='1800' b='0' i='0'")
    );
    let rectangle_text = text_shape(
        2,
        TEXT_RECT,
        &paragraph("Text in rectangle"),
        &solid("CCDDEE"),
        &outline("2060D0", 25_400),
    );
    let image_text = format!(
        "{}{}{}",
        text_shape(
            2,
            (914_400, 914_400, 5_486_400, 3_048_000),
            &paragraph("Hello Indonesia — Résumé"),
            &solid("E1EEFA"),
            &outline("2060D0", 25_400)
        ),
        picture(3, (1_524_000, 2_159_000, 914_400, 914_400)),
        text_shape(
            4,
            (2_743_200, 2_159_000, 3_175_000, 914_400),
            &paragraph("Text and image"),
            "<a:noFill/>",
            no_line
        )
    );
    let overlap = format!(
        "{}{}{}",
        text_shape(
            2,
            TEXT_RECT,
            &paragraph("Below rectangle"),
            "<a:noFill/>",
            no_line
        ),
        rectangle(
            3,
            (914_400, 914_400, 3_048_000, 914_400),
            &solid("F0B020"),
            no_line
        ),
        text_shape(
            4,
            TEXT_RECT,
            &paragraph("Above rectangle"),
            "<a:noFill/>",
            no_line
        )
    );
    vec![
        ("one-text-box.pptx", box_deck(&paragraph("Hello"))),
        ("unicode-text.pptx", box_deck(&unicode)),
        ("multiple-runs.pptx", box_deck(&runs)),
        (
            "multiple-paragraphs.pptx",
            box_deck(&format!(
                "{}<a:p><a:endParaRPr sz='1200'/></a:p>{}",
                paragraph("First paragraph"),
                paragraph("Third paragraph")
            )),
        ),
        ("styled-text-basic.pptx", box_deck(&styled)),
        ("text-in-rectangle.pptx", content_deck("", &rectangle_text)),
        ("text-with-image.pptx", image_deck(false, &image_text)),
        ("overlapping-text-shape.pptx", content_deck("", &overlap)),
    ]
}

pub fn font_fixtures() -> Vec<(&'static str, Parts)> {
    let paragraphs = "<a:p><a:r><a:rPr sz='2400'><a:latin typeface='O2P Test Sans'/></a:rPr><a:t>ASCII Résumé ffi ạ́ Привет Ελληνικά</a:t></a:r></a:p><a:p><a:r><a:rPr sz='2400'><a:latin typeface='O2P Test Sans'/></a:rPr><a:t>日本語</a:t></a:r></a:p><a:p><a:r><a:rPr sz='2400'><a:latin typeface='O2P Test Arabic'/></a:rPr><a:t>مرحبا</a:t></a:r></a:p>";
    vec![(
        "unicode-fonts.pptx",
        content_deck(
            "",
            &text_shape(
                2,
                (914400, 914400, 8229600, 3657600),
                paragraphs,
                "<a:noFill/>",
                "<a:ln><a:noFill/></a:ln>",
            ),
        ),
    )]
}

/// Original wrapping fixtures: direct fonts, outlined inset boxes, no autofit.
pub fn wrapping_fixtures() -> Vec<(&'static str, Parts)> {
    let run = |text: &str| {
        format!(
            "<a:r><a:rPr sz='2000'><a:latin typeface='O2P Test Sans'/></a:rPr><a:t>{text}</a:t></a:r>"
        )
    };
    let paragraph = |text: &str, alignment: &str| {
        format!("<a:p><a:pPr algn='{alignment}'/>{}</a:p>", run(text))
    };
    let deck = |text: &str, height: i64| {
        content_deck(
            "",
            &text_shape(
                2,
                (914_400, 914_400, 2_794_000, height),
                text,
                &solid("F1F5FA"),
                &outline("506080", 12_700),
            )
            .replace("wrap='none'", "wrap='square'"),
        )
    };
    let long = "Measured words wrap into several lines inside this box. Every line uses actual shaped glyph advances and preserves readable text.";
    let mixed = format!(
        "<a:p>{}<a:r><a:rPr sz='2400' b='1' i='1'><a:latin typeface='O2P Test Sans'/>{}</a:rPr><a:t>bold italic red words continue across several wrapped lines </a:t></a:r>{}</a:p>",
        run("Normal text leads into "),
        solid("C81428"),
        run("and normal text resumes here.")
    );
    vec![
        (
            "wrapping-short.pptx",
            deck(&paragraph("Short text", "l"), 3_810_000),
        ),
        ("wrapping-long.pptx", deck(&paragraph(long, "l"), 3_810_000)),
        (
            "multiple-paragraphs-wrapped.pptx",
            deck(
                &format!(
                    "{}<a:p><a:endParaRPr sz='1200'/></a:p>{}",
                    paragraph(
                        "First paragraph wraps with several measured words on each line.",
                        "l"
                    ),
                    paragraph(
                        "Another paragraph starts below the empty paragraph and also wraps.",
                        "l"
                    )
                ),
                4_572_000,
            ),
        ),
        ("mixed-runs-wrapped.pptx", deck(&mixed, 4_572_000)),
        (
            "centered-wrapped.pptx",
            deck(&paragraph(long, "ctr"), 3_810_000),
        ),
        (
            "right-aligned-wrapped.pptx",
            deck(&paragraph(long, "r"), 3_810_000),
        ),
        (
            "unicode-wrapped.pptx",
            deck(
                &format!(
                    "{}{}<a:p><a:r><a:rPr sz='2000'><a:latin typeface='O2P Test Arabic'/></a:rPr><a:t>مرحبا مرحبا مرحبا مرحبا مرحبا</a:t></a:r></a:p>",
                    paragraph("Résumé café ạ́ ffi Résumé café ạ́ ffi", "l"),
                    paragraph("日本語 日本語 日本語 日本語 日本語", "l")
                ),
                4_572_000,
            ),
        ),
        ("overflow-text.pptx", deck(&paragraph(long, "l"), 533_400)),
        (
            "long-unbroken-word.pptx",
            deck(
                &paragraph(
                    "SupercalifragilisticexpialidociousffiSupercalifragilisticexpialidocious",
                    "l",
                ),
                3_810_000,
            ),
        ),
    ]
}

/// Individual-element transforms; asymmetric text and the 2x2 color image
/// expose reflection axes and composition order without Office-generated data.
pub fn transform_fixtures() -> Vec<(&'static str, Parts)> {
    let paragraph = |label: &str| {
        format!(
            "<a:p><a:r><a:rPr sz='1800'><a:latin typeface='O2P Test Sans'/></a:rPr><a:t>{label}</a:t></a:r></a:p>"
        )
    };
    let bounds = (2_540_000, 1_905_000, 2_032_000, 1_016_000);
    let painted = |id, coords, label: &str| {
        text_shape(
            id,
            coords,
            &paragraph(label),
            &solid("DDEEFF"),
            &outline("205080", 25_400),
        )
        .replace("wrap='none'", "wrap='square'")
    };
    let transform = |element: String, attrs: &str| {
        element.replacen("<a:xfrm>", &format!("<a:xfrm {attrs}>"), 1)
    };
    let rect = rectangle(2, bounds, &solid("F0B020"), &outline("804020", 25_400));
    let ellipse = ellipse(2, bounds, &solid("40B080"), &outline("205040", 25_400));
    let image = picture(2, bounds);
    let h = transform(painted(2, bounds, "ABC flip H"), "flipH='1'");
    let v = transform(painted(2, bounds, "ABC flip V"), "flipV='true'");
    let hv_rot = transform(
        painted(2, bounds, "ABC both + 45"),
        "rot='2700000' flipH='true' flipV='1'",
    );
    let mixed = format!(
        "{}{}{}{}{}{}{}{}{}",
        transform(
            rectangle(
                2,
                (762000, 762000, 1524000, 762000),
                &solid("F0B020"),
                &outline("804020", 25400)
            ),
            "rot='2700000'"
        ),
        transform(
            painted(3, (3302000, 762000, 1778000, 762000), "Rectangle 90"),
            "rot='5400000'"
        ),
        transform(
            self::ellipse(
                4,
                (5842000, 762000, 2032000, 762000),
                &solid("40B080"),
                &outline("205040", 25400)
            ),
            "rot='2700000'"
        ),
        transform(
            picture(5, (762000, 2540000, 1524000, 1016000)),
            "rot='5400000'"
        ),
        transform(
            painted(6, (3048000, 2540000, 2286000, 1016000), "Rotated real text"),
            "rot='-900000'"
        ),
        transform(
            painted(7, (5842000, 2540000, 2032000, 1016000), "ABC horizontal"),
            "flipH='1'"
        ),
        transform(
            painted(8, (762000, 4699000, 2032000, 1016000), "ABC vertical"),
            "flipV='1'"
        ),
        transform(
            picture(9, (3556000, 4699000, 1524000, 1016000)),
            "flipH='1' rot='2700000'"
        ),
        transform(
            straight_line(
                10,
                (5842000, 4826000, 1778000, 762000),
                &outline("C02040", 38100)
            ),
            "rot='2700000' flipV='1'"
        )
    );
    vec![
        (
            "rotated-rectangle-45.pptx",
            content_deck("", &transform(rect.clone(), "rot='2700000'")),
        ),
        (
            "rotated-rectangle-90.pptx",
            content_deck("", &transform(rect, "rot='5400000'")),
        ),
        (
            "rotated-ellipse.pptx",
            content_deck("", &transform(ellipse, "rot='2700000'")),
        ),
        (
            "rotated-image.pptx",
            image_deck(false, &transform(image, "rot='5400000'")),
        ),
        (
            "rotated-text.pptx",
            content_deck(
                "",
                &transform(
                    painted(
                        2,
                        (bounds.0, bounds.1, bounds.2, 1_625_600),
                        "Résumé 日本語 ffi wraps with real selectable text.",
                    ),
                    "rot='2700000'",
                ),
            ),
        ),
        ("flipped-horizontal.pptx", content_deck("", &h)),
        ("flipped-vertical.pptx", content_deck("", &v)),
        ("flipped-and-rotated.pptx", content_deck("", &hv_rot)),
        ("mixed-transforms.pptx", image_deck(false, &mixed)),
    ]
}
