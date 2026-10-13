mod support;
use o2p_core::{DrawOp, GlyphTextOp, ParagraphSpacing, VerticalAlignment};
use o2p_fonts::{FontBook, FontWarning};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{
    ParseLimits, PptxError, Presentation, Shape, TextAlignment, TextBox, TextLayoutWarningKind,
};
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    Presentation::parse_with_content(
        &OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default())?,
        ParseLimits::default(),
    )
}
fn text(model: &Presentation) -> &TextBox {
    model.slide_contents()[0]
        .shapes()
        .iter()
        .find_map(|s| match s {
            Shape::TextBox(t) => Some(t),
            _ => None,
        })
        .unwrap()
}
fn fonts() -> FontBook {
    let mut book = FontBook::new();
    book.register_directory(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"),
    )
    .unwrap();
    book.set_fallback_families(vec!["O2P Test Sans".into(), "O2P Test Japanese".into()]);
    book
}
fn glyphs(model: &Presentation) -> Vec<GlyphTextOp> {
    model
        .to_display_list_with_fonts(&fonts())
        .unwrap()
        .display_list
        .pages
        .remove(0)
        .ops
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::GlyphText(t) => Some(t),
            _ => None,
        })
        .collect()
}
fn replace(parts: &mut Parts, part: &str, from: &str, to: &str) {
    let bytes = &mut parts.iter_mut().find(|(n, _)| n == part).unwrap().1;
    let xml = String::from_utf8(bytes.clone()).unwrap();
    assert!(xml.contains(from), "missing {from} in {part}");
    *bytes = xml.replace(from, to).into_bytes();
}
fn no_warning(model: &Presentation, feature: &str) {
    assert!(
        !model
            .warnings()
            .iter()
            .any(|w| format!("{w:?}").contains(feature)),
        "{:?}",
        model.warnings()
    );
}
fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

#[test]
fn fixtures_reproduce_and_render_deterministically_through_both_adapters() {
    for (name, parts) in fidelity2c_fixtures() {
        assert_eq!(
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../fixtures/pptx")
                    .join(name)
            )
            .unwrap(),
            zip(&parts, true, false)
        );
        let model = parse(&parts).unwrap();
        o2p_pdf::render_with_report(&model.to_display_list()).unwrap();
        let display = model
            .to_display_list_with_fonts(&fonts())
            .unwrap()
            .display_list;
        assert_eq!(
            o2p_pdf::render(&display).unwrap(),
            o2p_pdf::render(&display).unwrap()
        );
    }
}

#[test]
fn character_bullet_and_custom_font_preserve_unicode_and_pdf_selection() {
    for (name, glyph, family) in [
        ("bullet-char.pptx", "•", "O2P Test Sans"),
        ("bullet-custom-char.pptx", "»", "O2P Test Sans"),
        ("bullet-custom-font.pptx", "日", "O2P Test Japanese"),
    ] {
        let model = parse(&typography_fixture(name)).unwrap();
        let bullet = text(&model).paragraphs[0].bullet.as_ref().unwrap();
        assert_eq!(bullet.glyph, glyph);
        assert_eq!(bullet.font.family, family);
        assert_eq!(glyphs(&model)[0].text.source, glyph);
        let pdf = o2p_pdf::render(
            &model
                .to_display_list_with_fonts(&fonts())
                .unwrap()
                .display_list,
        )
        .unwrap();
        let doc = lopdf::Document::load_mem(&pdf).unwrap();
        assert!(doc.extract_text(&[1]).unwrap().contains(glyph));
        no_warning(&model, "buChar");
        no_warning(&model, "buFont");
    }
}

#[test]
fn missing_bullet_font_uses_existing_font_fallback_and_diagnostics() {
    let model = parse(&typography_deck(
        "",
        "<a:buChar char='日'/><a:buFont typeface='Absent Bullet Font'/>",
        "",
        "Text",
    ))
    .unwrap();
    let report = model.to_display_list_with_fonts(&fonts()).unwrap();
    assert!(report.warnings.iter().any(|w| matches!(&w.kind, TextLayoutWarningKind::Font(FontWarning::MissingFont { family }) if family == "Absent Bullet Font")));
    let bullet = glyphs(&model).remove(0);
    assert_eq!(bullet.text.source, "日");
    assert!(
        bullet
            .text
            .runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .all(|g| g.id != 0)
    );
}

#[test]
fn bullet_master_style_inheritance_and_local_character_override_are_independent() {
    let inherited = parse(&bullet_inherited("", "", "")).unwrap();
    assert_eq!(
        text(&inherited).paragraphs[0]
            .bullet
            .as_ref()
            .unwrap()
            .glyph,
        "•"
    );
    let local = parse(&bullet_inherited(
        "<a:pPr><a:buChar char='–'/></a:pPr>",
        "",
        "",
    ))
    .unwrap();
    let bullet = text(&local).paragraphs[0].bullet.as_ref().unwrap();
    assert_eq!(bullet.glyph, "–");
    assert_eq!(bullet.font.family, "O2P Test Sans");
    near(text(&local).paragraphs[0].format.margin_left, 27.0);
}

#[test]
fn bullet_layout_and_master_placeholder_properties_override_master_text_style() {
    for (layout, master, expected) in [
        (
            "<a:pPr><a:buChar char='»'/></a:pPr>",
            "<a:pPr><a:buChar char='–'/></a:pPr>",
            "»",
        ),
        ("", "<a:pPr><a:buChar char='–'/></a:pPr>", "–"),
    ] {
        let model = parse(&bullet_inherited("", layout, master)).unwrap();
        assert_eq!(
            text(&model).paragraphs[0].bullet.as_ref().unwrap().glyph,
            expected
        );
    }
}

#[test]
fn explicit_no_bullet_blocks_all_inherited_bullets() {
    for (local, layout, master) in [
        ("<a:pPr><a:buNone/></a:pPr>", "", ""),
        ("", "<a:pPr><a:buNone/></a:pPr>", ""),
        ("", "", "<a:pPr><a:buNone/></a:pPr>"),
    ] {
        let model = parse(&bullet_inherited(local, layout, master)).unwrap();
        assert!(text(&model).paragraphs[0].bullet.is_none());
        assert!(!glyphs(&model).iter().any(|r| r.text.source == "•"));
        no_warning(&model, "buNone");
    }
}

#[test]
fn bullet_font_tx_and_theme_alias_override_inherited_bullet_family() {
    let follow = parse(&bullet_inherited(
        "<a:pPr><a:buFontTx/><a:defRPr><a:latin typeface='O2P Test Japanese'/></a:defRPr></a:pPr>",
        "",
        "",
    ))
    .unwrap();
    assert_eq!(
        text(&follow).paragraphs[0]
            .bullet
            .as_ref()
            .unwrap()
            .font
            .family,
        "O2P Test Japanese"
    );
    let themed = parse(&bullet_inherited(
        "<a:pPr><a:buFont typeface='+mj-lt'/></a:pPr>",
        "",
        "",
    ))
    .unwrap();
    assert_eq!(
        text(&themed).paragraphs[0]
            .bullet
            .as_ref()
            .unwrap()
            .font
            .family,
        "O2P Test Sans"
    );
}

#[test]
fn paragraph_margins_indent_and_hanging_bullet_have_correct_sign_and_units() {
    for (name, margin, indent) in [
        ("bullet-char.pptx", 27.0, -18.0),
        ("bullet-indent.pptx", 36.0, 10.0),
        ("hanging-bullet.pptx", 27.0, -18.0),
    ] {
        let model = parse(&typography_fixture(name)).unwrap();
        let t = text(&model);
        let p = &t.paragraphs[0];
        near(p.format.margin_left, margin);
        near(p.format.first_line_indent, indent);
        let ops = glyphs(&model);
        near(
            ops[0].origin.x,
            t.rect.origin.x + t.insets.left + margin + indent,
        );
        near(ops[1].origin.x, t.rect.origin.x + t.insets.left + margin);
        near(ops[0].origin.y, ops[1].origin.y);
    }
}

#[test]
fn wrapped_bullet_continuations_align_to_text_margin_and_have_one_bullet() {
    let model = parse(&typography_fixture("wrapped-bullet.pptx")).unwrap();
    let ops = glyphs(&model);
    assert!(ops.len() > 3);
    assert_eq!(ops.iter().filter(|r| r.text.source == "•").count(), 1);
    let x = text(&model).rect.origin.x + text(&model).insets.left + 27.0;
    for run in ops.iter().skip(1) {
        near(run.origin.x, x);
    }
}

#[test]
fn indentation_inherits_per_property_and_explicit_zero_overrides() {
    let model = parse(&bullet_inherited(
        "<a:pPr marL='0'/>",
        "<a:pPr indent='-127000' marR='254000'/>",
        "<a:pPr marL='508000'/>",
    ))
    .unwrap();
    let p = &text(&model).paragraphs[0];
    near(p.format.margin_left, 0.0);
    near(p.format.first_line_indent, -10.0);
    near(p.format.margin_right, 20.0);
}

#[test]
fn title_and_body_alignment_inherit_and_local_alignment_wins() {
    let title = parse(&typography_fixture("centered-title-placeholder.pptx")).unwrap();
    assert_eq!(text(&title).paragraphs[0].alignment, TextAlignment::Center);
    assert_eq!(text(&title).vertical_alignment, VerticalAlignment::Center);
    for (local, layout, master, expected) in [
        ("", "<a:pPr algn='r'/>", "", TextAlignment::Right),
        ("", "", "<a:pPr algn='ctr'/>", TextAlignment::Center),
        (
            "<a:pPr algn='l'/>",
            "<a:pPr algn='r'/>",
            "",
            TextAlignment::Left,
        ),
    ] {
        let model = parse(&bullet_inherited(local, layout, master)).unwrap();
        assert_eq!(text(&model).paragraphs[0].alignment, expected);
    }
}

#[test]
fn point_spacing_is_applied_between_paragraph_boxes() {
    for (name, first_y, delta) in [
        ("paragraph-space-before.pptx", 105.6, 24.0),
        ("paragraph-space-after.pptx", 95.6, 32.0),
    ] {
        let model = parse(&typography_fixture(name)).unwrap();
        let ops = glyphs(&model);
        assert_eq!(ops.len(), 2);
        near(ops[0].origin.y, first_y);
        near(ops[1].origin.y - ops[0].origin.y, delta);
    }
}

#[test]
fn percent_spacing_normalizes_integer_and_strict_percent_forms() {
    for value in ["25000", "25%", "25.0%"] {
        let model = parse(&typography_deck(
            "",
            &format!("<a:spcBef><a:spcPct val='{value}'/></a:spcBef>"),
            "spcFirstLastPara='1'",
            "Text",
        ))
        .unwrap();
        assert_eq!(
            text(&model).paragraphs[0].format.space_before,
            ParagraphSpacing::TextMultiple(0.25)
        );
        near(glyphs(&model)[0].origin.y, 100.6);
    }
}

#[test]
fn spacing_inheritance_and_local_zero_preserve_independent_properties() {
    let model = parse(&bullet_inherited(
        "<a:pPr><a:spcBef><a:spcPts val='0'/></a:spcBef></a:pPr>",
        "<a:pPr><a:spcAft><a:spcPct val='20000'/></a:spcAft></a:pPr>",
        "",
    ))
    .unwrap();
    let p = &text(&model).paragraphs[0];
    assert_eq!(p.format.space_before, ParagraphSpacing::Points(0.0));
    assert_eq!(p.format.space_after, ParagraphSpacing::TextMultiple(0.2));
    let inherited = parse(&typography_fixture("inherited-paragraph-spacing.pptx")).unwrap();
    assert_eq!(
        text(&inherited).paragraphs[0].format.space_before,
        ParagraphSpacing::Points(5.0)
    );
}

#[test]
fn default_edge_spacing_is_suppressed_and_explicit_true_is_respected() {
    for (body, expected) in [
        ("", 95.6),
        ("spcFirstLastPara='0'", 95.6),
        ("spcFirstLastPara='1'", 105.6),
    ] {
        let model = parse(&typography_deck(
            "",
            "<a:spcBef><a:spcPts val='1000'/></a:spcBef>",
            body,
            "Text",
        ))
        .unwrap();
        near(glyphs(&model)[0].origin.y, expected);
    }
}

#[test]
fn vertical_anchor_positions_use_measured_height_and_insets() {
    for (anchor, alignment, offset) in [
        ("t", VerticalAlignment::Top, 0.0),
        ("ctr", VerticalAlignment::Center, 45.0),
        ("b", VerticalAlignment::Bottom, 90.0),
    ] {
        let model = parse(&typography_deck(
            "",
            "",
            &format!("anchor='{anchor}' tIns='127000' bIns='254000'"),
            "Text",
        ))
        .unwrap();
        let t = text(&model);
        assert_eq!(t.vertical_alignment, alignment);
        near(
            glyphs(&model)[0].origin.y,
            t.rect.origin.y + 10.0 + 20.0 + offset,
        );
        no_warning(&model, "vertical anchor");
    }
}

#[test]
fn inherited_anchor_and_edge_spacing_resolve_layout_before_master_and_local_zero() {
    let mut parts = bullet_inherited("", "", "");
    replace(
        &mut parts,
        MASTER_PART,
        "<a:bodyPr/>",
        "<a:bodyPr anchor='b' spcFirstLastPara='1'/>",
    );
    let master = parse(&parts).unwrap();
    assert_eq!(text(&master).vertical_alignment, VerticalAlignment::Bottom);
    assert!(text(&master).include_edge_spacing);
    replace(
        &mut parts,
        LAYOUT_PART,
        "<a:bodyPr/>",
        "<a:bodyPr anchor='ctr'/>",
    );
    let layout = parse(&parts).unwrap();
    assert_eq!(text(&layout).vertical_alignment, VerticalAlignment::Center);
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:bodyPr/>",
        "<a:bodyPr anchor='t' spcFirstLastPara='0'/>",
    );
    let local = parse(&parts).unwrap();
    assert_eq!(text(&local).vertical_alignment, VerticalAlignment::Top);
    assert!(!text(&local).include_edge_spacing);
    no_warning(&master, "bodyPr anchor");
    no_warning(&layout, "vertical anchor");
}

#[test]
fn rotated_anchored_text_retains_local_layout_and_transform_scope() {
    let parts = typography_fixture("textbox-anchor-rotated.pptx");
    let model = parse(&parts).unwrap();
    let report = model.to_display_list_with_fonts(&fonts()).unwrap();
    assert!(matches!(
        report.display_list.pages[0].ops[0],
        DrawOp::BeginTransform(_)
    ));
    assert!(matches!(
        report.display_list.pages[0].ops.last(),
        Some(DrawOp::EndTransform)
    ));
    let mut plain = parts;
    replace(&mut plain, "ppt/slides/slide1.xml", " rot='5400000'", "");
    assert_eq!(glyphs(&model), glyphs(&parse(&plain).unwrap()));
    o2p_pdf::render(&report.display_list).unwrap();
}

#[test]
fn unsupported_spacing_and_autofit_remain_structured_warnings() {
    let mut parts = typography_deck(
        "",
        "<a:spcBef><a:unknownSpacing val='100'/></a:spcBef><a:lnSpc><a:spcPct val='120000'/></a:lnSpc>",
        "",
        "Text",
    );
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:noAutofit/>",
        "<a:spAutoFit/>",
    );
    let model = parse(&parts).unwrap();
    for feature in ["unsupported paragraph spacing", "lnSpc", "spAutoFit"] {
        assert!(
            model
                .warnings()
                .iter()
                .any(|w| format!("{w:?}").contains(feature))
        );
    }
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 20.0);
}

#[test]
fn malformed_paragraph_values_return_owned_errors() {
    for (attrs, children, body) in [
        ("marL='-1'", "", ""),
        ("indent='no'", "", ""),
        ("indent='51206401'", "", ""),
        ("", "<a:buChar/>", ""),
        ("", "<a:buChar char=''/>", ""),
        ("", "<a:buFont/>", ""),
        ("", "<a:buChar char='•'/><a:buNone/>", ""),
        ("", "<a:spcBef/>", ""),
        ("", "<a:spcBef><a:spcPts val='-1'/></a:spcBef>", ""),
        ("", "<a:spcAft><a:spcPct val='NaN%'/></a:spcAft>", ""),
        ("", "", "spcFirstLastPara='yes'"),
    ] {
        assert!(
            parse(&typography_deck(attrs, children, body, "Text")).is_err(),
            "{attrs} {children} {body}"
        );
    }
}

#[test]
fn combining_unicode_bullet_shapes_without_losing_the_source_string() {
    let model = parse(&typography_deck("", "<a:buChar char='é'/>", "", "Text")).unwrap();
    assert_eq!(glyphs(&model)[0].text.source, "é");
    let pdf = o2p_pdf::render(
        &model
            .to_display_list_with_fonts(&fonts())
            .unwrap()
            .display_list,
    )
    .unwrap();
    assert!(
        lopdf::Document::load_mem(&pdf)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains("é")
    );
}

#[test]
fn inherited_bullet_font_can_be_overridden_without_replacing_the_bullet() {
    let model = parse(&bullet_inherited(
        "<a:pPr><a:buFont typeface='O2P Test Japanese'/></a:pPr>",
        "",
        "",
    ))
    .unwrap();
    let b = text(&model).paragraphs[0].bullet.as_ref().unwrap();
    assert_eq!(b.glyph, "•");
    assert_eq!(b.font.family, "O2P Test Japanese");
}

#[test]
fn local_list_defaults_and_paragraph_level_select_bullet_indent_and_spacing_together() {
    let mut parts = bullet_inherited("<a:pPr lvl='2'/>", "", "");
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:lstStyle/>",
        "<a:lstStyle><a:lvl3pPr marL='635000' indent='-127000' algn='r'><a:buChar char='»'/><a:spcAft><a:spcPts val='900'/></a:spcAft></a:lvl3pPr></a:lstStyle>",
    );
    let model = parse(&parts).unwrap();
    let p = &text(&model).paragraphs[0];
    assert_eq!(p.bullet.as_ref().unwrap().glyph, "»");
    near(p.format.margin_left, 50.0);
    near(p.format.first_line_indent, -10.0);
    assert_eq!(p.format.space_after, ParagraphSpacing::Points(9.0));
    assert_eq!(p.alignment, TextAlignment::Right);
}

#[test]
fn unsupported_numbering_blocks_an_inherited_character_bullet_and_warns() {
    let model = parse(&bullet_inherited(
        "<a:pPr><a:buAutoNum type='arabicPeriod'/></a:pPr>",
        "",
        "",
    ))
    .unwrap();
    assert!(text(&model).paragraphs[0].bullet.is_none());
    assert!(
        model
            .warnings()
            .iter()
            .any(|w| format!("{w:?}").contains("buAutoNum"))
    );
}

#[test]
fn inherited_spacing_between_multiple_paragraphs_and_strict_namespaces_are_supported() {
    let mut parts = bullet_inherited("", "", "");
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        "</p:txBody>",
        "<a:p><a:r><a:t>Second</a:t></a:r></a:p></p:txBody>",
    );
    let model = parse(&parts).unwrap();
    let runs = glyphs(&model);
    // 24pt line + inherited 5pt before the second paragraph; no first-edge gap.
    near(runs[2].origin.y - runs[0].origin.y, 29.0);
    for (_, bytes) in &mut parts {
        if let Ok(xml) = std::str::from_utf8(bytes) {
            *bytes = xml
                .replace(DML, "http://purl.oclc.org/ooxml/drawingml/main")
                .replace(PML, "http://purl.oclc.org/ooxml/presentationml/main")
                .replace(
                    OFFICE_RELS,
                    "http://purl.oclc.org/ooxml/officeDocument/relationships",
                )
                .into_bytes();
        }
    }
    let strict = parse(&parts).unwrap();
    assert_eq!(text(&model), text(&strict));
}
