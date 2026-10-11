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
