mod support;

use o2p_core::{DrawOp, GlyphTextOp};
use o2p_fonts::{FontBook, FontWarning};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{FontDisplayReport, ParseLimits, Presentation, Shape, TextLayoutWarningKind};
use std::path::PathBuf;
use support::*;

fn fonts() -> FontBook {
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

fn parse(parts: &Parts) -> Presentation {
    let package =
        OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default()).unwrap()
}

fn texts(report: &FontDisplayReport) -> Vec<&GlyphTextOp> {
    report.display_list.pages[0]
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::GlyphText(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn lines<'a>(runs: &[&'a GlyphTextOp]) -> Vec<Vec<&'a GlyphTextOp>> {
    let mut lines: Vec<Vec<&GlyphTextOp>> = vec![];
    for &run in runs {
        if lines
            .last()
            .is_none_or(|line| line[0].origin.y != run.origin.y)
        {
            lines.push(vec![]);
        }
        lines.last_mut().unwrap().push(run);
    }
    lines
}

#[test]
fn nine_fixtures_are_reproducible_wrapped_styled_selectable_and_deterministic() {
    let fonts = fonts();
    for (name, parts) in wrapping_fixtures() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(
            std::fs::read(path).unwrap(),
            zip(&parts, true, false),
            "{name}"
        );
        let presentation = parse(&parts);
        let report = presentation.to_display_list_with_fonts(&fonts).unwrap();
        assert_eq!(
            report,
            presentation.to_display_list_with_fonts(&fonts).unwrap()
        );
        let text_box = presentation.slide_contents()[0]
            .shapes()
            .iter()
            .find_map(|shape| match shape {
                Shape::TextBox(t) => Some(t),
                _ => None,
            })
            .unwrap();
        let expected: String = text_box
            .paragraphs
            .iter()
            .flat_map(|p| &p.runs)
            .map(|r| r.text.as_str())
            .collect();
        let runs = texts(&report);
        let logical: String = runs.iter().map(|r| r.text.source.as_str()).collect();
        assert_eq!(logical, expected, "{name}");
        let lines = lines(&runs);
        assert_eq!(lines.len() == 1, name == "wrapping-short.pptx", "{name}");
        let left = text_box.rect.origin.x + text_box.insets.left;
        let width = text_box.rect.size.width - text_box.insets.left - text_box.insets.right;
        for line in &lines {
            let measured: f64 = line.iter().map(|r| f64::from(r.text.advance.x)).sum();
            assert!(measured <= f64::from(width), "{name}: {measured} > {width}");
            let factor = match name {
                "centered-wrapped.pptx" => 0.5,
                "right-aligned-wrapped.pptx" => 1.0,
                _ => 0.0,
            };
            assert!(
                (f64::from(line[0].origin.x)
                    - (f64::from(left) + (f64::from(width) - measured) * factor))
                    .abs()
                    < 0.00002,
                "{name}"
            );
        }
        assert!(report.warnings.iter().all(|w| !matches!(
            w.kind,
            TextLayoutWarningKind::Font(FontWarning::MissingGlyph { .. })
        )));
        let overflow: Vec<_> = report
            .warnings
            .iter()
            .filter(|w| w.kind == TextLayoutWarningKind::Overflow)
            .collect();
        assert_eq!(overflow.len(), usize::from(name == "overflow-text.pptx"));
        if let Some(warning) = overflow.first() {
            assert_eq!((warning.page, warning.text_box), (0, 0));
            assert!(
                runs.last().unwrap().origin.y > text_box.rect.origin.y + text_box.rect.size.height
            );
        }
        if name == "mixed-runs-wrapped.pptx" {
            let red_lines = lines
                .iter()
                .filter(|line| {
                    line.iter()
                        .any(|r| r.color == o2p_core::Color::rgb(200, 20, 40))
                })
                .count();
            assert!(red_lines >= 2);
        }
        // Shapes still paint before all wrapped text within this shape.
        assert!(matches!(
            report.display_list.pages[0].ops[0],
            DrawOp::Rect(_)
        ));
        assert!(matches!(
            report.display_list.pages[0].ops[1],
            DrawOp::StrokeRect(_)
        ));
        let bytes = o2p_pdf::render(&report.display_list).unwrap();
        assert_eq!(bytes, o2p_pdf::render(&report.display_list).unwrap());
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(
            pdf.objects
                .values()
                .any(|o| o.as_dict().is_ok_and(|d| d.has(b"FontFile2")))
        );
        assert!(
            pdf.objects
                .values()
                .any(|o| o.as_dict().is_ok_and(|d| d.has(b"ToUnicode")))
        );
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        let actual_text: Vec<_> = content
            .operations
            .iter()
            .filter(|op| op.operator == "BDC")
            .map(|op| {
                let encoded = op.operands[1]
                    .as_dict()
                    .unwrap()
                    .get(b"ActualText")
                    .unwrap()
                    .as_str()
                    .unwrap();
                assert_eq!(&encoded[..2], &[0xfe, 0xff]);
                String::from_utf16(
                    &encoded[2..]
                        .chunks_exact(2)
                        .map(|b| u16::from_be_bytes([b[0], b[1]]))
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(actual_text.concat(), expected, "{name}: ActualText");
        let extracted = pdf.extract_text(&[1]).unwrap();
        if name == "unicode-wrapped.pptx" {
            // lopdf ignores ActualText for continuation glyphs in multi-glyph
            // clusters; the complete logical Unicode is verified above.
            assert!(extracted.contains("Résumé"));
            assert!(extracted.contains("日本語"));
        } else {
            assert!(!extracted.contains('\u{fffd}'), "{name}");
        }
        if name == "mixed-runs-wrapped.pptx" {
            let compact: String = extracted.chars().filter(|c| !c.is_whitespace()).collect();
            assert!(compact.contains("bolditalicredwordscontinueacrossseveralwrappedlines"));
        }
    }
}

#[test]
fn insets_paragraphs_and_trailing_breaks_reach_generic_layout() {
    let fonts = fonts();
    let shape = text_shape(2, TEXT_RECT,
        "<a:p><a:pPr><a:defRPr sz='1000'/></a:pPr><a:r><a:t>First</a:t></a:r><a:br/><a:endParaRPr sz='2000'/></a:p><a:p><a:endParaRPr sz='1200'/></a:p><a:p><a:r><a:rPr sz='1000'/><a:t>Last</a:t></a:r></a:p>",
        "<a:noFill/>", "<a:ln><a:noFill/></a:ln>")
        .replace("wrap='none'", "wrap='square' lIns='12700' rIns='25400' tIns='38100' bIns='50800'");
    let report = parse(&content_deck("", &shape))
        .to_display_list_with_fonts(&fonts)
        .unwrap();
    let runs = texts(&report);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].origin.x, 73.0);
    assert_eq!(runs[0].origin.y, 85.0);
    assert!((runs[1].origin.y - runs[0].origin.y - 50.4).abs() < 0.00002);
}

#[test]
fn unsupported_body_and_typography_remain_structured_warnings() {
    let shape = text_shape(2, TEXT_RECT,
        "<a:p><a:pPr><a:buChar char='•'/><a:buAutoNum type='arabicPeriod'/><a:lnSpc><a:spcPct val='150000'/></a:lnSpc></a:pPr><a:r><a:rPr spc='100' u='sng' strike='sngStrike' baseline='30000'/><a:t>Unsupported styles</a:t></a:r></a:p>",
        "<a:noFill/>", "<a:ln><a:noFill/></a:ln>")
        .replace("<a:noAutofit/>", "<a:normAutofit fontScale='50000'/><a:spAutoFit/>");
    let presentation = parse(&content_deck("", &shape));
    for feature in [
        "wrapping disabled",
        "buChar",
        "buAutoNum",
        "lnSpc",
        "@spc",
        "@u",
        "@strike",
        "@baseline",
        "normAutofit",
        "spAutoFit",
    ] {
        assert!(
            presentation
                .warnings()
                .iter()
                .any(|w| w.to_string().contains(feature)),
            "{feature}"
        );
    }
    let report = presentation.to_display_list_with_fonts(&fonts()).unwrap();
    assert_eq!(texts(&report)[0].text.source, "Unsupported styles");
}

#[test]
fn empty_text_boxes_preserve_shapes_and_blank_paragraphs_need_no_fonts() {
    let shape = text_shape(
        2,
        TEXT_RECT,
        "<a:p/>",
        &solid("DDEEFF"),
        &outline("334455", 12_700),
    )
    .replace("wrap='none'", "wrap='square'");
    let presentation = parse(&content_deck("", &shape));
    let report = presentation
        .to_display_list_with_fonts(&FontBook::new())
        .unwrap();
    assert!(texts(&report).is_empty());
    assert!(report.warnings.is_empty());
    assert!(matches!(
        report.display_list.pages[0].ops.as_slice(),
        [DrawOp::Rect(_), DrawOp::StrokeRect(_)]
    ));
    assert!(o2p_pdf::render(&report.display_list).is_ok());
}
