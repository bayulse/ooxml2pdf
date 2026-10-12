mod support;
use o2p_core::{DrawOp, GlyphTextOp};
use o2p_fonts::{FontBook, FontWarning};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{ParseLimits, Presentation, Shape, TextLayoutWarningKind};
use std::path::PathBuf;
use support::*;

fn registry() -> FontBook {
    let mut fonts = FontBook::new();
    fonts
        .register_directory(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
        .unwrap();
    fonts.set_fallback_families(vec![
        "O2P Test Sans".into(),
        "O2P Test Japanese".into(),
        "O2P Test Arabic".into(),
    ]);
    fonts
}
fn parse(paragraph: &str) -> Presentation {
    let parts = content_deck(
        "",
        &text_shape(
            2,
            TEXT_RECT,
            paragraph,
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        ),
    );
    let package =
        OoxmlPackage::from_bytes(&zip(&parts, true, false), SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default()).unwrap()
}
fn texts(display: &o2p_core::DisplayList) -> Vec<&GlyphTextOp> {
    display.pages[0]
        .ops
        .iter()
        .filter_map(|op| {
            if let DrawOp::GlyphText(t) = op {
                Some(t)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn direct_family_local_defaults_styles_and_theme_font_warning() {
    let presentation = parse(
        "<a:p><a:pPr><a:defRPr sz='2400'><a:latin typeface='O2P Test Sans'/></a:defRPr></a:pPr><a:r><a:rPr b='1' i='true'/><a:t>Résumé ffi</a:t></a:r><a:r><a:rPr><a:latin typeface='Missing Family'/></a:rPr><a:t>日本語 😀</a:t></a:r></a:p>",
    );
    let text = presentation.slide_contents()[0]
        .shapes()
        .iter()
        .find_map(|shape| {
            if let Shape::TextBox(t) = shape {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        text.paragraphs[0].runs[0].style.family.as_deref(),
        Some("O2P Test Sans")
    );
    assert_eq!(
        text.paragraphs[0].runs[1].style.family.as_deref(),
        Some("Missing Family")
    );
    let report = presentation
        .to_display_list_with_fonts(&registry())
        .unwrap();
    let ops = texts(&report.display_list);
    assert_eq!(ops[0].text.font_size, 24.0);
    let face = o2p_fonts::FontFace::from_resource(ops[0].text.runs[0].font.clone()).unwrap();
    assert_eq!(face.metadata().weight, 700);
    assert!(face.metadata().italic);
    assert!(report.warnings.iter().any(|w| matches!(
        w.kind,
        TextLayoutWarningKind::Font(FontWarning::MissingFont { .. })
    )));
    assert!(report.warnings.iter().any(|w| matches!(
        w.kind,
        TextLayoutWarningKind::Font(FontWarning::MissingGlyph { character: '😀' })
    )));
    let theme =
        parse("<a:p><a:r><a:rPr><a:latin typeface='+mj-lt'/></a:rPr><a:t>A</a:t></a:r></a:p>");
    assert!(
        theme
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("theme font +mj-lt unresolved"))
    );
    assert!(
        parse(
            "<a:p><a:r><a:rPr><a:latin typeface='O2P Test Sans'/></a:rPr><a:t>A</a:t></a:r></a:p>"
        )
        .warnings()
        .iter()
        .all(|w| !w.to_string().contains("unsupported latin"))
    );
}

#[test]
fn alignment_measures_shaped_advances_and_explicit_break_layout_is_retained() {
    let fonts = registry();
    for (alignment, factor) in [("l", 0.0), ("ctr", 0.5), ("r", 1.0)] {
        let presentation = parse(&format!(
            "<a:p><a:pPr algn='{alignment}'/><a:r><a:rPr sz='2000'/><a:t>ffi</a:t></a:r><a:r><a:rPr sz='2400'/><a:t>日本語</a:t></a:r><a:br/><a:r><a:t>Next</a:t></a:r></a:p>"
        ));
        let display = presentation
            .to_display_list_with_fonts(&fonts)
            .unwrap()
            .display_list;
        let ops = texts(&display);
        assert_eq!(ops.len(), 3);
        let total = ops[0].text.advance.x + ops[1].text.advance.x;
        assert!((ops[0].origin.x - (72.0 + 7.2 + (360.0 - 14.4 - total) * factor)).abs() < 0.001);
        assert!((ops[1].origin.x - ops[0].origin.x - ops[0].text.advance.x).abs() < 0.001);
        assert_eq!(ops[0].origin.y, ops[1].origin.y);
        assert!(ops[2].origin.y > ops[1].origin.y);
    }
}

#[test]
fn unicode_fixture_and_existing_text_image_shape_paint_order_are_preserved() {
    let fonts = registry();
    for (name, parts) in font_fixtures() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(std::fs::read(path).unwrap(), zip(&parts, true, false));
        let package =
            OoxmlPackage::from_bytes(&zip(&parts, true, false), SecurityLimits::default()).unwrap();
        let presentation =
            Presentation::parse_with_content(&package, ParseLimits::default()).unwrap();
        let report = presentation.to_display_list_with_fonts(&fonts).unwrap();
        assert!(report.warnings.iter().all(|w| !matches!(
            w.kind,
            TextLayoutWarningKind::Font(FontWarning::MissingGlyph { .. })
        )));
        assert_eq!(texts(&report.display_list)[1].text.source, "日本語");
        assert!(o2p_pdf::render(&report.display_list).is_ok());
    }
    for name in [
        "text-with-image.pptx",
        "overlapping-text-shape.pptx",
        "text-in-rectangle.pptx",
        "multiple-runs.pptx",
        "styled-text-basic.pptx",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        let package = OoxmlPackage::open(path, SecurityLimits::default()).unwrap();
        let presentation =
            Presentation::parse_with_content(&package, ParseLimits::default()).unwrap();
        let old = presentation.to_display_list();
        let new = presentation
            .to_display_list_with_fonts(&fonts)
            .unwrap()
            .display_list;
        assert_eq!(old.pages[0].size, new.pages[0].size);
        assert_eq!(old.pages[0].ops.len(), new.pages[0].ops.len());
        for (old, new) in old.pages[0].ops.iter().zip(&new.pages[0].ops) {
            match (old, new) {
                (DrawOp::Text(old), DrawOp::GlyphText(new)) => {
                    assert_eq!(old.text, new.text.source);
                    assert_eq!(old.font_size, new.text.font_size);
                    assert_eq!(old.color, new.color);
                    assert_eq!(old.origin.y, new.origin.y);
                }
                _ => assert_eq!(old, new),
            }
        }
        let bytes = o2p_pdf::render(&new).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(
            pdf.extract_text(&[1]).unwrap().contains("Text")
                || pdf.extract_text(&[1]).unwrap().contains("Styled")
                || !pdf.extract_text(&[1]).unwrap().trim().is_empty()
        );
    }
}
