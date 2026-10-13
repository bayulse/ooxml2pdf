mod support;
use o2p_core::{Color, DrawOp, Font, Point, TextOp};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{
    ParseLimits, PptxError, PptxWarning, Presentation, Shape, TextAlignment, TextBox, emu_to_points,
};
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    let package =
        OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default())
}
fn model(paragraphs: &str) -> Presentation {
    parse(&parts(paragraphs)).unwrap()
}
fn parts(paragraphs: &str) -> Parts {
    content_deck(
        "",
        &text_shape(
            2,
            TEXT_RECT,
            paragraphs,
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        ),
    )
}
fn text_box(model: &Presentation) -> &TextBox {
    model.slide_contents()[0]
        .shapes()
        .iter()
        .find_map(|s| {
            if let Shape::TextBox(t) = s {
                Some(t)
            } else {
                None
            }
        })
        .unwrap()
}
fn ops(model: &Presentation) -> Vec<TextOp> {
    model
        .to_display_list()
        .pages
        .remove(0)
        .ops
        .into_iter()
        .filter_map(|op| {
            if let DrawOp::Text(t) = op {
                Some(t)
            } else {
                None
            }
        })
        .collect()
}
fn warning(model: &Presentation, expected: &str) {
    assert!(model.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedSlideContent{slide_id:256,part,name} if part == "ppt/slides/slide1.xml" && name.contains(expected))),"{expected}: {:?}",model.warnings());
}

#[test]
fn fixtures_are_reproducible_and_render_through_both_adapters() {
    for (name, parts) in text_fixtures() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(std::fs::read(path).unwrap(), zip(&parts, true, false));
        let model = parse(&parts).unwrap();
        let display = model.to_display_list();
        let report = o2p_pdf::render_with_report(&display).unwrap();
        assert_eq!(report, o2p_pdf::render_with_report(&display).unwrap());
        let pdf = lopdf::Document::load_mem(&report.bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 1);
        assert!(!pdf.extract_text(&[1]).unwrap().trim().is_empty());
        assert_eq!(report.warnings.is_empty(), name != "unicode-text.pptx");
    }
}

#[test]
fn literal_unicode_entities_cdata_whitespace_and_run_order_are_preserved() {
    let model = model(
        "<a:p><a:r><a:t xml:space='preserve'> Hello &amp; &lt;World&gt; </a:t></a:r><a:r><a:t><![CDATA[Résumé 日本語]]></a:t></a:r></a:p>",
    );
    let runs = &text_box(&model).paragraphs[0].runs;
    assert_eq!(
        runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        [" Hello & <World> ", "Résumé 日本語"]
    );
    assert_eq!(
        ops(&model)
            .iter()
            .map(|op| op.text.as_str())
            .collect::<Vec<_>>(),
        [" Hello & <World> ", "Résumé 日本語"]
    );
}

#[test]
fn direct_size_boolean_style_and_srgb_color_reach_core_and_pdf() {
    let model = model(
        "<a:p><a:r><a:rPr sz='2450' b='true' i='1'><a:solidFill><a:srgbClr val='12AbCD'/></a:solidFill></a:rPr><a:t>Styled</a:t></a:r></a:p>",
    );
    let style = &text_box(&model).paragraphs[0].runs[0].style;
    assert_eq!(style.font_size, 24.5);
    assert!(style.bold && style.italic);
    assert_eq!(style.color, Color::rgb(18, 171, 205));
    let operation = &ops(&model)[0];
    assert_eq!(operation.font, Font::SansSerifBoldItalic);
    assert_eq!(operation.font_size, 24.5);
    assert_eq!(operation.color, style.color);
}

#[test]
fn paragraph_defaults_apply_locally_and_direct_false_overrides_true() {
    let model = model(
        "<a:p><a:pPr><a:defRPr sz='2000' b='1' i='true'><a:solidFill><a:srgbClr val='FF0000'/></a:solidFill></a:defRPr></a:pPr><a:r><a:rPr b='0' i='false'/><a:t>Regular</a:t></a:r><a:r><a:t>Bold italic</a:t></a:r></a:p><a:p><a:r><a:t>Fallback</a:t></a:r></a:p>",
    );
    let operations = ops(&model);
    assert_eq!(operations[0].font, Font::SansSerif);
    assert_eq!(operations[0].font_size, 20.0);
    assert_eq!(operations[0].color, Color::rgb(255, 0, 0));
    assert_eq!(operations[1].font, Font::SansSerifBoldItalic);
    assert_eq!(operations[2].font_size, 18.0);
    assert_eq!(operations[2].color, Color::BLACK);
    warning(&model, "text inheritance/font resolution");
}

#[test]
fn box_transform_dimensions_insets_and_baseline_use_points() {
    let shape = text_shape(
        2,
        NONTRIVIAL_RECT,
        "<a:p><a:r><a:rPr sz='2000'/><a:t>Hello</a:t></a:r></a:p>",
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    )
    .replace(
        "wrap='none'",
        "wrap='none' lIns='12700' rIns='25400' tIns='38100' bIns='50800'",
    );
    let model = parse(&content_deck("", &shape)).unwrap();
    let text = text_box(&model);
    let (x, y, w, h) = NONTRIVIAL_RECT;
    assert_eq!(
        text.rect.origin,
        Point {
            x: emu_to_points(x),
            y: emu_to_points(y)
        }
    );
    assert_eq!(text.rect.size.width, emu_to_points(w));
    assert_eq!(text.rect.size.height, emu_to_points(h));
    assert_eq!(
        (
            text.insets.left,
            text.insets.right,
            text.insets.top,
            text.insets.bottom
        ),
        (1.0, 2.0, 3.0, 4.0)
    );
    let run = &ops(&model)[0];
    assert_eq!(
        run.origin,
        Point {
            x: emu_to_points(x) + 1.0,
            y: emu_to_points(y) + 23.0
        }
    );
}

#[test]
fn runs_advance_by_the_same_fixed_metrics_used_for_alignment() {
    let model = model(
        "<a:p><a:r><a:rPr sz='1000'/><a:t>Hello</a:t></a:r><a:r><a:rPr sz='2000' b='1'/><a:t>Résumé</a:t></a:r></a:p>",
    );
    let operations = ops(&model);
    assert!((operations[1].origin.x - operations[0].origin.x - 22.78).abs() < 0.0001);
    assert_eq!(operations[0].origin.y, operations[1].origin.y);
    assert!((operations[0].origin.y - 95.6).abs() < 0.0001); // 72 + top inset + max size
}

#[test]
fn paragraphs_blank_paragraphs_and_end_properties_have_deterministic_spacing() {
    let model = model(
        "<a:p><a:r><a:rPr sz='1000'/><a:t>First</a:t></a:r><a:endParaRPr sz='4000'/></a:p><a:p><a:endParaRPr sz='2000'/></a:p><a:p><a:r><a:rPr sz='1000'/><a:t>Third</a:t></a:r></a:p>",
    );
    assert_eq!(text_box(&model).paragraphs.len(), 3);
    assert!(text_box(&model).paragraphs[1].runs.is_empty());
    assert_eq!(text_box(&model).paragraphs[0].end_style.font_size, 40.0);
    let operations = ops(&model);
    assert_eq!(
        operations
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        ["First", "Third"]
    );
    assert!((operations[1].origin.y - operations[0].origin.y - 36.0).abs() < 0.0001);
    assert_eq!(operations[0].font_size, 10.0); // endParaRPr must not restyle the run
}

#[test]
fn explicit_breaks_and_literal_newlines_restart_at_left() {
    let model = model(
        "<a:p><a:pPr><a:defRPr sz='1000'/></a:pPr><a:r><a:t>One</a:t></a:r><a:br/><a:r><a:t>Two\nThree</a:t></a:r></a:p>",
    );
    let operations = ops(&model);
    assert_eq!(
        operations
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        ["One", "Two", "Three"]
    );
    assert_eq!(operations[0].origin.x, operations[1].origin.x);
    assert!((operations[1].origin.y - operations[0].origin.y - 12.0).abs() < 0.0001);
    assert!((operations[2].origin.y - operations[1].origin.y - 12.0).abs() < 0.0001);
}

#[test]
fn alignment_uses_the_combined_unwrapped_line_width() {
    for (alignment, expected) in [
        ("l", TextAlignment::Left),
        ("ctr", TextAlignment::Center),
        ("r", TextAlignment::Right),
    ] {
        let model = model(&format!(
            "<a:p><a:pPr algn='{alignment}'><a:defRPr sz='1000'/></a:pPr><a:r><a:t>Hello</a:t></a:r><a:r><a:t> World</a:t></a:r></a:p>"
        ));
        assert_eq!(text_box(&model).paragraphs[0].alignment, expected);
        let line_width = Font::SansSerif.text_width("Hello World", 10.0) as f32;
        let offset = match expected {
            TextAlignment::Left => 0.0,
            TextAlignment::Center => (360.0 - 14.4 - line_width) / 2.0,
            TextAlignment::Right => 360.0 - 14.4 - line_width,
        };
        assert!((ops(&model)[0].origin.x - (72.0 + 7.2 + offset)).abs() < 0.0001);
    }
    let model = model("<a:p><a:pPr algn='just'/><a:r><a:t>Justified</a:t></a:r></a:p>");
    warning(&model, "unsupported paragraph alignment");
    assert_eq!(
        text_box(&model).paragraphs[0].alignment,
        TextAlignment::Left
    );
}

#[test]
fn text_only_placeholders_keep_explicit_geometry_when_layout_is_missing() {
    let shape = text_shape(
        2,
        TEXT_RECT,
        "<a:p><a:r><a:t>Explicit position</a:t></a:r></a:p>",
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    )
    .replace("<a:prstGeom prst='rect'><a:avLst/></a:prstGeom>", "")
    .replace("<p:nvPr/>", "<p:nvPr><p:ph/></p:nvPr>");
    let model = parse(&content_deck("", &shape)).unwrap();
    assert_eq!(model.slide_contents()[0].shapes().len(), 1);
    assert_eq!(ops(&model)[0].text, "Explicit position");
    warning(&model, "missing slideLayout relationship");
    warning(&model, "unmatched placeholder");
}

#[test]
fn fill_stroke_text_and_mixed_element_order_are_preserved() {
    let fixtures = text_fixtures();
    let model = parse(
        &fixtures
            .iter()
            .find(|(n, _)| *n == "text-in-rectangle.pptx")
            .unwrap()
            .1,
    )
    .unwrap();
    let display = model.to_display_list();
    assert!(matches!(
        display.pages[0].ops.as_slice(),
        [DrawOp::Rect(_), DrawOp::StrokeRect(_), DrawOp::Text(_)]
    ));
    let overlap = parse(
        &fixtures
            .iter()
            .find(|(n, _)| *n == "overlapping-text-shape.pptx")
            .unwrap()
            .1,
    )
    .unwrap();
    assert!(matches!(
        overlap.to_display_list().pages[0].ops.as_slice(),
        [DrawOp::Text(_), DrawOp::Rect(_), DrawOp::Text(_)]
    ));
    let mixed = parse(
        &fixtures
            .iter()
            .find(|(n, _)| *n == "text-with-image.pptx")
            .unwrap()
            .1,
    )
    .unwrap();
    assert!(matches!(
        mixed.to_display_list().pages[0].ops.as_slice(),
        [
            DrawOp::Rect(_),
            DrawOp::Rect(_),
            DrawOp::StrokeRect(_),
            DrawOp::Text(_),
            DrawOp::Image(_),
            DrawOp::Text(_)
        ]
    ));
    let shape = text_shape(
        2,
        TEXT_RECT,
        "<a:p><a:r><a:t>Ellipse text</a:t></a:r></a:p>",
        &solid("FF0000"),
        &outline("000000", 12_700),
    )
    .replace("prst='rect'", "prst='ellipse'");
    let ellipse = parse(&content_deck("", &shape)).unwrap();
    assert!(matches!(
        ellipse.to_display_list().pages[0].ops.as_slice(),
        [
            DrawOp::Ellipse(_),
            DrawOp::StrokeEllipse(_),
            DrawOp::Text(_)
        ]
    ));
}

#[test]
fn overflow_is_warned_and_still_renders_without_autofit_or_wrapping() {
    let shape = text_shape(
        2,
        (0, 0, 12_700, 12_700),
        "<a:p><a:r><a:t>Long text</a:t></a:r></a:p>",
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    );
    let model = parse(&content_deck("", &shape)).unwrap();
    warning(&model, "text box overflow");
    assert_eq!(ops(&model)[0].text, "Long text");
    assert!(o2p_pdf::render(&model.to_display_list()).is_ok());
}

#[test]
fn unsupported_recognized_styles_emit_structured_warnings() {
    for (property, expected) in [
        ("<a:rPr u='sng'/>", "@u"),
        ("<a:rPr strike='sngStrike'/>", "@strike"),
        ("<a:rPr spc='100'/>", "@spc"),
        ("<a:rPr baseline='30000'/>", "@baseline"),
        ("<a:rPr><a:ea typeface='Calibri'/></a:rPr>", "ea"),
        (
            "<a:rPr><a:solidFill><a:schemeClr val='tx1'/></a:solidFill></a:rPr>",
            "schemeClr",
        ),
        (
            "<a:rPr><a:solidFill><a:srgbClr val='FF0000'><a:alpha val='50000'/></a:srgbClr></a:solidFill></a:rPr>",
            "alpha",
        ),
    ] {
        let model = model(&format!("<a:p><a:r>{property}<a:t>Text</a:t></a:r></a:p>"));
        warning(&model, expected);
        assert_eq!(ops(&model).len(), 1);
        assert_eq!(ops(&model)[0].color, Color::BLACK);
    }
    let model = model(
        "<a:p><a:pPr><a:buChar char='•'/><a:lnSpc><a:spcPct val='200000'/></a:lnSpc></a:pPr><a:r><a:t>Literal text only</a:t></a:r></a:p>",
    );
    assert_eq!(
        text_box(&model).paragraphs[0]
            .bullet
            .as_ref()
            .unwrap()
            .glyph,
        "•"
    );
    warning(&model, "lnSpc");
}

#[test]
fn body_features_warn_and_vertical_or_rotated_text_is_omitted_but_shape_survives() {
    let shape = text_shape(
        2,
        TEXT_RECT,
        "<a:p><a:r><a:t>Hello</a:t></a:r></a:p>",
        &solid("FF0000"),
        "<a:ln><a:noFill/></a:ln>",
    );
    for (body, warning_name, rendered) in [
        ("wrap='unknown'", "text wrapping", true),
        ("anchor='just'", "vertical anchor", true),
        ("vert='vert'", "vertical text", false),
        ("rot='60000'", "text rotation", false),
    ] {
        let model = parse(&content_deck("", &shape.replace("wrap='none'", body))).unwrap();
        warning(&model, warning_name);
        assert_eq!(!ops(&model).is_empty(), rendered);
        assert!(matches!(
            model.to_display_list().pages[0].ops[0],
            DrawOp::Rect(_)
        ));
    }
    let model = parse(&content_deck(
        "",
        &shape.replace("<a:noAutofit/>", "<a:normAutofit fontScale='50000'/>"),
    ))
    .unwrap();
    warning(&model, "normAutofit");
}

#[test]
fn malformed_supported_markup_returns_owned_errors() {
    for markup in [
        "<a:p><a:r/></a:p>",
        "<a:p><a:r><a:t>x</a:t><a:t>y</a:t></a:r></a:p>",
        "<a:p><a:r><a:t><a:r/></a:t></a:r></a:p>",
        "<a:p><a:r>stray<a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr sz='0'/><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr sz='400001'/><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr sz='NaN'/><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr b='yes'/><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr i='2'/><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr/><a:rPr/><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:rPr><a:solidFill><a:srgbClr val='FFF'/></a:solidFill></a:rPr><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:endParaRPr/><a:r><a:t>x</a:t></a:r></a:p>",
        "<a:p><a:r><a:t>x</a:t></a:r><a:pPr/></a:p>",
        "<a:p><a:pPr/><a:pPr/></a:p>",
        "<a:p><a:endParaRPr/><a:endParaRPr/></a:p>",
        "<a:p><a:r><a:t>x&#9;y</a:t></a:r></a:p>",
        "",
    ] {
        assert!(
            matches!(parse(&parts(markup)), Err(PptxError::InvalidSlide { .. })),
            "{markup}"
        );
    }
    let mut parts = parts("<a:p/>");
    let slide = parts
        .iter_mut()
        .find(|(n, _)| n == "ppt/slides/slide1.xml")
        .unwrap();
    slide.1 = String::from_utf8(slide.1.clone())
        .unwrap()
        .replace("</a:p>", "")
        .replace("<a:p/>", "<a:p>")
        .into_bytes();
    assert!(matches!(parse(&parts), Err(PptxError::InvalidXml { .. })));
}

#[test]
fn strict_namespaces_utf16_and_xml_limits_apply_to_text() {
    let mut parts = parts("<a:p><a:r><a:t>Résumé 日本語</a:t></a:r></a:p>");
    let expected = parse(&parts).unwrap();
    let source = String::from_utf8(
        parts
            .iter()
            .find(|(n, _)| n == "ppt/slides/slide1.xml")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        source
            .replace(PML, "http://purl.oclc.org/ooxml/presentationml/main")
            .replace(DML, "http://purl.oclc.org/ooxml/drawingml/main")
            .into_bytes(),
    );
    assert_eq!(parse(&parts).unwrap(), expected);
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(source.encode_utf16().flat_map(u16::to_le_bytes));
    replace(&mut parts, "ppt/slides/slide1.xml", utf16);
    assert_eq!(parse(&parts).unwrap(), expected);
    let package =
        OoxmlPackage::from_bytes(&zip(&parts, true, false), SecurityLimits::default()).unwrap();
    assert!(matches!(
        Presentation::parse_with_content(
            &package,
            ParseLimits {
                max_xml_depth: 6,
                ..ParseLimits::default()
            }
        ),
        Err(PptxError::LimitExceeded { .. })
    ));
}

#[test]
fn trailing_blank_line_uses_end_paragraph_style() {
    let model = model(
        "<a:p><a:pPr><a:defRPr sz='1000'/></a:pPr><a:r><a:t>First</a:t></a:r><a:br/><a:endParaRPr sz='2000'/></a:p><a:p><a:r><a:rPr sz='1000'/><a:t>Next</a:t></a:r></a:p>",
    );
    let operations = ops(&model);
    assert!((operations[1].origin.y - operations[0].origin.y - 36.0).abs() < 0.0001);
}

#[test]
fn duplicate_text_bodies_and_malformed_body_properties_fail() {
    let shape = text_shape(
        2,
        TEXT_RECT,
        "<a:p/>",
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    );
    for shape in [
        shape.replace("</p:sp>", "<p:txBody><a:p/></p:txBody></p:sp>"),
        shape.replace("wrap='none'", "lIns='-1'"),
        shape.replace("wrap='none'", "tIns='2147483648'"),
        shape.replace("wrap='none'", "rot='NaN'"),
        shape.replace("<a:lstStyle/>", "<a:lstStyle/><a:lstStyle/>"),
    ] {
        assert!(matches!(
            parse(&content_deck("", &shape)),
            Err(PptxError::InvalidSlide { .. })
        ));
    }
}
