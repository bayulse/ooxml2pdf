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
