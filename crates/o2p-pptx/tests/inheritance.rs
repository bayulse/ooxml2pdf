mod support;
use o2p_core::{Color, DrawOp};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{
    Fill, ParseLimits, PptxError, PptxWarning, Presentation, Shape, TextAlignment, TextBox,
};
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    Presentation::parse_with_content(
        &OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default())?,
        ParseLimits::default(),
    )
}
fn fixture(name: &str) -> Parts {
    inheritance_fixtures()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
fn text(model: &Presentation, index: usize) -> &TextBox {
    model.slide_contents()[0]
        .shapes()
        .iter()
        .filter_map(|s| {
            if let Shape::TextBox(t) = s {
                Some(t)
            } else {
                None
            }
        })
        .nth(index)
        .unwrap()
}
fn warning(model: &Presentation, needle: &str) -> bool {
    model.warnings().iter().any(
        |w| matches!(w, PptxWarning::UnsupportedSlideContent {name, ..} if name.contains(needle)),
    )
}
fn rewrite(parts: &mut Parts, name: &str, from: &str, to: &str) {
    let bytes = &mut parts.iter_mut().find(|(n, _)| n == name).unwrap().1;
    let s = String::from_utf8(bytes.clone()).unwrap();
    assert!(s.contains(from), "{name}: {from}");
    *bytes = s.replace(from, to).into_bytes();
}
fn simple(local: &str, layout: &str, master: &str) -> Parts {
    inheritance_deck("", local, "", layout, "", master)
}

#[test]
fn fixtures_are_reproducible_and_owned() {
    for (name, parts) in inheritance_fixtures() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(
            std::fs::read(path).unwrap(),
            zip(&parts, true, false),
            "{name}"
        );
        let result = parse(&parts);
        if name.starts_with("malformed-") {
            assert!(matches!(
                result,
                Err(PptxError::InvalidSlideRelationship { .. })
            ));
        } else {
            let model = result.unwrap();
            assert_eq!(model.clone(), model);
            assert!(o2p_pdf::render_with_report(&model.to_display_list()).is_ok());
        }
    }
}
#[test]
fn slide_layout_master_relationships_and_idx_identity() {
    let model = parse(&fixture("multiple-placeholders.pptx")).unwrap();
    assert_eq!(model.slide_layouts()[0].part_name(), LAYOUT_PART);
    assert_eq!(
        model.slide_layouts()[0].master_part_name(),
        Some(MASTER_PART)
    );
    assert_eq!(model.slide_masters()[0].part_name(), MASTER_PART);
    assert_eq!(model.slide_contents()[0].placeholders()[1].idx(), Some(2));
    assert_eq!(text(&model, 0).rect.origin.x, 72.0);
    assert_eq!(text(&model, 1).rect.origin.x, 200.0);
    assert_eq!(text(&model, 1).rect.origin.y, 300.0);
}
#[test]
fn inherited_xy_and_partial_local_override() {
    let model = parse(&fixture("layout-inherited-position.pptx")).unwrap();
    assert_eq!(text(&model, 0).rect.origin.x, 72.0);
    assert_eq!(text(&model, 0).rect.origin.y, 100.0);
    let model = parse(&fixture("local-overrides-layout.pptx")).unwrap();
    assert_eq!(text(&model, 0).rect.origin.x, 200.0);
    assert_eq!(text(&model, 0).rect.origin.y, 100.0);
}
#[test]
fn inherited_width_height_and_partial_extent_override() {
    let model = parse(&fixture("layout-inherited-size.pptx")).unwrap();
    assert_eq!(text(&model, 0).rect.size.width, 400.0);
    assert_eq!(text(&model, 0).rect.size.height, 100.0);
    let model = parse(&fixture("local-overrides-master.pptx")).unwrap();
    assert_eq!(text(&model, 0).rect.size.width, 300.0);
    assert_eq!(text(&model, 0).rect.size.height, 100.0);
}
#[test]
fn master_position_and_type_matching_normalize_title_and_body() {
    let model = parse(&fixture("master-inherited-position.pptx")).unwrap();
    assert_eq!(text(&model, 0).rect.origin.x, 72.0);
    for (layout_type, master_type) in [
        ("ctrTitle", "title"),
        ("subTitle", "body"),
        ("obj", "body"),
        ("object", "body"),
        ("text", "body"),
    ] {
        let local = placeholder(
            2,
            layout_type,
            Some(4),
            "",
            Some(&inherited_paragraph("Visible", "")),
        );
        let layout = placeholder(2, layout_type, Some(4), "", None);
        let master = placeholder(2, master_type, Some(99), FULL_GEOMETRY, None);
        let model = parse(&simple(&local, &layout, &master)).unwrap();
        assert_eq!(text(&model, 0).rect.origin.y, 100.0, "{layout_type}");
    }
}
#[test]
fn missing_idx_uses_unique_type_when_no_default_idx_candidate_exists() {
    let local = placeholder(
        2,
        "title",
        None,
        "",
        Some(&inherited_paragraph("Title", "")),
    );
    let layout = placeholder(2, "title", Some(5), FULL_GEOMETRY, None);
    let model = parse(&simple(&local, &layout, "")).unwrap();
    assert_eq!(text(&model, 0).rect.origin.x, 72.0);
}
#[test]
fn explicit_unmatched_idx_and_ambiguous_matches_do_not_guess() {
    let local = placeholder(
        2,
        "body",
        Some(9),
        "",
        Some(&inherited_paragraph("Missing", "")),
    );
    let parent = placeholder(2, "body", Some(1), FULL_GEOMETRY, None);
    let model = parse(&simple(&local, &parent, "")).unwrap();
    assert!(model.slide_contents()[0].shapes().is_empty());
    assert!(warning(&model, "unmatched placeholder"));
    let local = placeholder(
        2,
        "body",
        Some(1),
        "",
        Some(&inherited_paragraph("Ambiguous", "")),
    );
    let model = parse(&simple(&local, &format!("{parent}{parent}"), "")).unwrap();
    assert!(warning(&model, "ambiguous placeholder"));
    assert!(model.slide_contents()[0].shapes().is_empty());
}
#[test]
fn shape_geometry_never_matches_by_position() {
    let local = placeholder(
        2,
        "body",
        Some(1),
        "",
        Some(&inherited_paragraph("Unmatched", "")),
    );
    let deco = rectangle(2, NONTRIVIAL_RECT, &solid("FFFFFF"), "");
    let model = parse(&simple(&local, &deco, "")).unwrap();
    assert!(warning(&model, "incomplete placeholder geometry"));
    assert!(
        model.slide_contents()[0].placeholders()[0]
            .geometry()
            .x
            .is_none()
    );
}
#[test]
fn inherited_fill_outline_and_partial_line_override() {
    let paragraph = inherited_paragraph("Paint", "");
    let local = placeholder(2, "body", Some(1), "<a:ln w='38100'/>", Some(&paragraph));
    let layout = placeholder(
        2,
        "body",
        Some(1),
        &format!(
            "{FULL_GEOMETRY}{}{}",
            solid("DDEEFF"),
            outline("2060D0", 12700)
        ),
        None,
    );
    let model = parse(&simple(&local, &layout, "")).unwrap();
    assert!(
        matches!(model.slide_contents()[0].shapes()[0], Shape::Rectangle {fill:Some(Fill::Solid(c)), stroke:Some(s), ..} if c==Color::rgb(221,238,255) && s.width==3.0 && s.color==Color::rgb(32,96,208))
    );
    let local = placeholder(
        2,
        "body",
        Some(1),
        "<a:noFill/><a:ln><a:noFill/></a:ln>",
        Some(&paragraph),
    );
    let model = parse(&simple(&local, &layout, "")).unwrap();
    assert!(
        !model.to_display_list().pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Rect(_) | DrawOp::StrokeRect(_)))
    );
}
#[test]
fn inherited_font_size_family_color_bold_italic_alignment() {
    let model = parse(&fixture("inherited-text-style.pptx")).unwrap();
    let paragraph = &text(&model, 0).paragraphs[0];
    let style = &paragraph.runs[0].style;
    assert_eq!(style.family.as_deref(), Some("O2P Test Sans"));
    assert_eq!(style.font_size, 28.0);
    assert_eq!(style.color, Color::rgb(32, 96, 208));
    assert!(style.bold && style.italic);
    assert_eq!(paragraph.alignment, TextAlignment::Center);
}
#[test]
fn local_run_and_paragraph_overrides_layout_and_master_per_property() {
    let mut parts = fixture("local-overrides-master.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:r><a:t>",
        "<a:r><a:rPr sz='1900' i='1'/><a:t>",
    );
    let model = parse(&parts).unwrap();
    let p = &text(&model, 0).paragraphs[0];
    let s = &p.runs[0].style;
    assert_eq!(s.font_size, 19.0);
    assert!(!s.bold && s.italic);
    assert_eq!(s.family.as_deref(), Some("O2P Test Sans"));
    assert_eq!(p.alignment, TextAlignment::Right);
}
#[test]
fn layout_over_master_precedence_keeps_unoverridden_properties() {
    let local = placeholder(
        2,
        "body",
        Some(1),
        "",
        Some(&inherited_paragraph("Effective", "")),
    );
    let layout = placeholder(
        2,
        "body",
        Some(1),
        "<a:xfrm><a:off x='2540000'/></a:xfrm>",
        Some(&inherited_paragraph(
            "Sample",
            "<a:pPr><a:defRPr sz='2000' b='0'/></a:pPr>",
        )),
    );
    let master = placeholder(
        2,
        "body",
        None,
        FULL_GEOMETRY,
        Some(&inherited_paragraph("Master", INHERITED_STYLE)),
    );
    let model = parse(&simple(&local, &layout, &master)).unwrap();
    let t = text(&model, 0);
    let s = &t.paragraphs[0].runs[0].style;
    assert_eq!(t.rect.origin.x, 200.0);
    assert_eq!(t.rect.origin.y, 100.0);
    assert_eq!(s.font_size, 20.0);
    assert!(!s.bold && s.italic);
    assert_eq!(s.color, Color::rgb(32, 96, 208));
}
#[test]
fn backgrounds_follow_slide_layout_master_order_and_explicit_unsupported_blocks_parent() {
    for (local, layout, expected) in [
        ("", "", Color::rgb(17, 34, 51)),
        ("", "445566", Color::rgb(68, 85, 102)),
        ("778899", "445566", Color::rgb(119, 136, 153)),
    ] {
        let model = parse(&inheritance_deck(
            &if local.is_empty() {
                String::new()
            } else {
                background(local)
            },
            "",
            &if layout.is_empty() {
                String::new()
            } else {
                background(layout)
            },
            "",
            &background("112233"),
            "",
        ))
        .unwrap();
        assert_eq!(
            model.slide_contents()[0].background(),
            Some(Fill::Solid(expected))
        );
    }
    let model = parse(&inheritance_deck(
        "<p:bg><p:bgRef idx='1001'/></p:bg>",
        "",
        "",
        "",
        &background("112233"),
        "",
    ))
    .unwrap();
    assert_eq!(model.slide_contents()[0].background(), None);
    assert!(warning(&model, "bgRef"));
}
#[test]
fn decorative_layers_preserve_xml_order_without_placeholder_content() {
    let draw = |id, color| {
        rectangle(
            id,
            (0, 0, 914400, 914400),
            &solid(color),
            "<a:ln><a:noFill/></a:ln>",
        )
    };
    let parts = inheritance_deck(
        &background("FFFFFF"),
        &draw(2, "0000FF"),
        "",
        &format!("{}{}", draw(2, "00FF00"), draw(3, "FFFF00")),
        &background("CCCCCC"),
        &format!("{}{}", draw(2, "FF0000"), draw(3, "FF00FF")),
    );
    let model = parse(&parts).unwrap();
    let list = model.to_display_list();
    let colors: Vec<_> = list.pages[0]
        .ops
        .iter()
        .map(|op| {
            if let DrawOp::Rect(r) = op {
                r.color
            } else {
                panic!("rectangle")
            }
        })
        .collect();
    assert_eq!(
        colors,
        vec![
            Color::WHITE,
            Color::rgb(255, 0, 0),
            Color::rgb(255, 0, 255),
            Color::rgb(0, 255, 0),
            Color::rgb(255, 255, 0),
            Color::rgb(0, 0, 255)
        ]
    );
    assert_eq!(
        o2p_pdf::render(&list).unwrap(),
        o2p_pdf::render(&parse(&parts).unwrap().to_display_list()).unwrap()
    );
    let model = parse(&fixture("placeholder-no-duplication.pptx")).unwrap();
    let pdf =
        lopdf::Document::load_mem(&o2p_pdf::render(&model.to_display_list()).unwrap()).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert_eq!(extracted.matches("Slide content").count(), 1);
    assert!(!extracted.contains("sample"));
}
#[test]
fn master_and_layout_decorative_primitives_use_existing_renderers() {
    let drawings = format!(
        "{}{}{}{}{}",
        rectangle(2, NONTRIVIAL_RECT, &solid("FF0000"), ""),
        ellipse(3, NONTRIVIAL_RECT, &solid("00FF00"), ""),
        straight_line(4, NONTRIVIAL_RECT, &outline("0000FF", 12700)),
        picture(5, NONTRIVIAL_RECT),
        text_shape(
            6,
            TEXT_RECT,
            &inherited_paragraph("Decorative text", ""),
            "",
            ""
        )
    );
    for master in [true, false] {
        let mut parts = if master {
            simple("", "", &drawings)
        } else {
            simple("", &drawings, "")
        };
        parts.push(("ppt/media/synthetic.png".into(), raster(false)));
        parts.push((
            if master {
                "ppt/slideMasters/_rels/master.xml.rels"
            } else {
                "ppt/slideLayouts/_rels/layout.xml.rels"
            }
            .into(),
            rels(&format!(
                "{}{}",
                if master {
                    String::new()
                } else {
                    relation("rMaster", "slideMaster", "../slideMasters/master.xml")
                },
                relation("rImage", "image", "../media/synthetic.png")
            )),
        ));
        // Replace rather than add the existing layout relationship part.
        if !master {
            let last = parts.pop().unwrap();
            replace(&mut parts, &last.0, last.1);
        }
        let model = parse(&parts).unwrap();
        let ops = &model.to_display_list().pages[0].ops;
        assert!(ops.iter().any(|o| matches!(o, DrawOp::Ellipse(_))));
        assert!(ops.iter().any(|o| matches!(o, DrawOp::Line(_))));
        assert!(ops.iter().any(|o| matches!(o, DrawOp::Image(_))));
        assert!(
            ops.iter()
                .any(|o| matches!(o,DrawOp::Text(t) if t.text=="Decorative text"))
        );
        assert!(o2p_pdf::render(&model.to_display_list()).is_ok());
    }
}
#[test]
fn show_master_shapes_suppresses_only_decorations_and_hidden_shapes() {
    for part in ["ppt/slides/slide1.xml", LAYOUT_PART] {
        let mut parts = fixture("master-decorative-shape.pptx");
        let root = if part == LAYOUT_PART {
            "p:sldLayout"
        } else {
            "p:sld"
        };
        rewrite(
            &mut parts,
            part,
            &format!("<{root} "),
            &format!("<{root} showMasterSp='false' "),
        );
        let model = parse(&parts).unwrap();
        assert_eq!(text(&model, 0).rect.origin.x, 72.0);
        assert_eq!(model.slide_contents()[0].shapes().len(), 1);
    }
}
#[test]
fn missing_layout_and_master_warn_with_owned_context() {
    for (part, needle) in [
        ("ppt/slides/_rels/slide1.xml.rels", "missing slideLayout"),
        (
            "ppt/slideLayouts/_rels/layout.xml.rels",
            "missing slideMaster",
        ),
    ] {
        let mut parts = fixture("inherited-text-style.pptx");
        replace(&mut parts, part, rels(""));
        let model = parse(&parts).unwrap();
        assert!(warning(&model, needle));
    }
}
#[test]
fn unsupported_placeholder_type_is_preserved_and_warned() {
    let mut parts = fixture("inherited-text-style.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "type='body'",
        "type='dt'",
    );
    let model = parse(&parts).unwrap();
    assert_eq!(model.slide_contents()[0].placeholders()[0].kind(), "dt");
    assert!(warning(&model, "unsupported placeholder type dt"));
}
#[test]
fn malformed_links_external_links_duplicate_links_fail_without_fetching() {
    for part in [
        "ppt/slides/_rels/slide1.xml.rels",
        "ppt/slideLayouts/_rels/layout.xml.rels",
    ] {
        let mut parts = fixture("inherited-text-style.pptx");
        rewrite(
            &mut parts,
            part,
            "<Relationship Id=",
            "<Relationship TargetMode='External' Id=",
        );
        assert!(matches!(
            parse(&parts),
            Err(PptxError::InvalidSlideRelationship { .. })
        ));
    }
    let mut parts = fixture("inherited-text-style.pptx");
    replace(
        &mut parts,
        "ppt/slides/_rels/slide1.xml.rels",
        rels(&format!(
            "{}{}",
            relation("a", "slideLayout", "../slideLayouts/layout.xml"),
            relation("b", "slideLayout", "../slideLayouts/layout.xml")
        )),
    );
    assert!(matches!(
        parse(&parts),
        Err(PptxError::InvalidSlideRelationship { .. })
    ));
}
#[test]
fn master_theme_integrates_with_inherited_fonts_colors_and_embedded_unicode() {
    let mut parts = fixture("layout-master-inheritance.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "Layout and master inheritance",
        "Résumé 日本語",
    );
    let model = parse(&parts).unwrap();
    assert!(model.theme().is_none());
    assert!(model.slide_masters()[0].theme().is_some());
    assert_eq!(
        text(&model, 0).paragraphs[0].runs[0]
            .style
            .family
            .as_deref(),
        Some("O2P Test Sans")
    );
    assert_eq!(
        text(&model, 0).paragraphs[0].runs[0].style.color,
        Color::rgb(32, 96, 208)
    );
    let mut fonts = o2p_fonts::FontBook::new();
    fonts
        .register_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"),
        )
        .unwrap();
    fonts.set_fallback_families(vec!["O2P Test Sans".into(), "O2P Test Japanese".into()]);
    let list = model
        .to_display_list_with_fonts(&fonts)
        .unwrap()
        .display_list;
    let bytes = o2p_pdf::render(&list).unwrap();
    assert!(bytes.windows(10).any(|w| w == b"/FontFile2"));
    assert!(bytes.windows(10).any(|w| w == b"/ToUnicode"));
    let extracted = lopdf::Document::load_mem(&bytes)
        .unwrap()
        .extract_text(&[1])
        .unwrap();
    assert!(extracted.contains("Résumé 日本語"));
}
#[test]
fn layout_master_xml_limits_malformed_values_and_metadata_only_regression() {
    for part in [LAYOUT_PART, MASTER_PART] {
        let mut parts = fixture("inherited-text-style.pptx");
        rewrite(&mut parts, part, "<p:cSld>", "<p:cSld><p:extLst><p:ext>");
        assert!(matches!(parse(&parts), Err(PptxError::InvalidXml { .. })));
        let package =
            OoxmlPackage::from_bytes(&zip(&parts, true, false), SecurityLimits::default()).unwrap();
        assert!(Presentation::parse(&package, ParseLimits::default()).is_ok());
        assert_eq!(package.inspect_pptx().unwrap().slide_count(), 1);
    }
    let mut parts = fixture("layout-inherited-position.pptx");
    rewrite(&mut parts, LAYOUT_PART, "x='914400'", "x='NaN'");
    assert!(matches!(parse(&parts), Err(PptxError::InvalidSlide { .. })));
}

#[test]
fn sparse_fill_font_and_outline_precedence_is_property_specific() {
    let master = placeholder(
        2,
        "body",
        None,
        &format!(
            "{FULL_GEOMETRY}{}{}",
            solid("112233"),
            outline("223344", 12700)
        ),
        Some(&inherited_paragraph("master", INHERITED_STYLE)),
    );
    let layout = placeholder(
        2,
        "body",
        Some(1),
        &format!("{}<a:ln w='25400'/>", solid("445566")),
        Some(&inherited_paragraph(
            "layout",
            "<a:pPr><a:defRPr sz='2200'><a:latin typeface='Layout Family'/></a:defRPr></a:pPr>",
        )),
    );
    for (paint, expected) in [
        ("", Color::rgb(68, 85, 102)),
        (solid("778899").as_str(), Color::rgb(119, 136, 153)),
    ] {
        let local = placeholder(
            2,
            "body",
            Some(1),
            paint,
            Some(&inherited_paragraph("slide", "")),
        );
        let model = parse(&simple(&local, &layout, &master)).unwrap();
        assert!(
            matches!(model.slide_contents()[0].shapes()[0],Shape::Rectangle{fill:Some(Fill::Solid(c)),stroke:Some(s),..} if c==expected && s.width==2.0 && s.color==Color::rgb(34,51,68))
        );
        let s = &text(&model, 0).paragraphs[0].runs[0].style;
        assert_eq!(s.family.as_deref(), Some("Layout Family"));
        assert_eq!(s.font_size, 22.0);
        assert!(s.bold && s.italic);
    }
}

#[test]
fn level_zero_defaults_merge_and_local_paragraph_styles_do_not_leak() {
    let mut layout = placeholder(
        2,
        "body",
        Some(1),
        FULL_GEOMETRY,
        Some(&inherited_paragraph("sample", "")),
    );
    layout=layout.replace("<a:lstStyle/>","<a:lstStyle><a:defPPr><a:defRPr sz='1800' b='1'/></a:defPPr><a:lvl1pPr algn='r'><a:defRPr sz='2400' i='1'/></a:lvl1pPr></a:lstStyle>");
    let local = placeholder(
        2,
        "body",
        Some(1),
        "",
        Some(&format!(
            "{}{}",
            inherited_paragraph(
                "First",
                "<a:pPr algn='l'><a:defRPr sz='1900' b='0'/></a:pPr>"
            ),
            inherited_paragraph("Second", "")
        )),
    );
    let model = parse(&simple(&local, &layout, "")).unwrap();
    let paras = &text(&model, 0).paragraphs;
    assert_eq!(paras[0].runs[0].style.font_size, 19.0);
    assert!(!paras[0].runs[0].style.bold);
    assert_eq!(paras[0].alignment, TextAlignment::Left);
    assert_eq!(paras[1].runs[0].style.font_size, 24.0);
    assert!(paras[1].runs[0].style.bold && paras[1].runs[0].style.italic);
    assert_eq!(paras[1].alignment, TextAlignment::Right);
}

#[test]
fn unsupported_ancestor_semantics_warn_and_do_not_guess() {
    let local = placeholder(
        2,
        "body",
        Some(1),
        "<a:gradFill/>",
        Some(&inherited_paragraph("slide", "")),
    );
    let layout = placeholder(
        2,
        "body",
        Some(1),
        &format!("{FULL_GEOMETRY}{}", solid("FF0000")),
        Some(&inherited_paragraph("sample", "")),
    )
    .replace(
        "<a:lstStyle/>",
        "<a:lstStyle><a:lvl2pPr><a:defRPr sz='9900'/></a:lvl2pPr></a:lstStyle>",
    )
    .replace("<a:bodyPr/>", "<a:bodyPr anchor='ctr'/>");
    let mut parts = simple(&local, &layout, "");
    rewrite(
        &mut parts,
        MASTER_PART,
        "</p:sldMaster>",
        "<p:txStyles><p:bodyStyle/></p:txStyles></p:sldMaster>",
    );
    let model = parse(&parts).unwrap();
    assert!(warning(&model, "gradFill"));
    assert!(!warning(&model, "bodyPr"));
    assert_eq!(
        text(&model, 0).vertical_alignment,
        o2p_core::VerticalAlignment::Center
    );
    assert!(
        !model.to_display_list().pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Rect(_)))
    );
    assert!(!warning(&model, "lvl2pPr") && !warning(&model, "txStyles"));
    assert_eq!(text(&model, 0).paragraphs[0].runs[0].style.font_size, 18.0);
}

#[test]
fn hidden_decorations_and_placeholders_keep_inheritance_and_layout_visible() {
    let mut parts = fixture("layout-master-inheritance.pptx");
    rewrite(
        &mut parts,
        MASTER_PART,
        "id='2' name='Placeholder'",
        "id='2' name='Placeholder' hidden='true'",
    );
    rewrite(
        &mut parts,
        MASTER_PART,
        "name='Rectangle 5'",
        "name='Rectangle 5' hidden='true'",
    );
    let model = parse(&parts).unwrap();
    assert_eq!(
        text(&model, 0).paragraphs[0].runs[0].style.color,
        Color::rgb(32, 96, 208)
    );
    assert!(
        model.to_display_list().pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Ellipse(_)))
    );
    assert!(model.slide_masters()[0].decorative_shapes().is_empty());
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "id='2' name='Placeholder'",
        "id='2' name='Placeholder' hidden='true'",
    );
    let model = parse(&parts).unwrap();
    assert_eq!(model.slide_contents()[0].placeholders().len(), 2);
    assert!(
        !model.to_display_list().pages[0]
            .ops
            .iter()
            .any(|op| matches!(op,DrawOp::Text(t) if t.text.contains("Layout and master")))
    );
    let mut parts = fixture("layout-master-inheritance.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<p:sld ",
        "<p:sld showMasterSp='0' ",
    );
    let model = parse(&parts).unwrap();
    assert!(
        model.to_display_list().pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Ellipse(_)))
    );
    assert!(model.slide_contents()[0].background().is_some());
}

#[test]
fn inherited_rotation_and_flips_use_existing_shared_transform_scope() {
    let mut parts = fixture("inherited-text-style.pptx");
    rewrite(
        &mut parts,
        LAYOUT_PART,
        "<a:xfrm>",
        "<a:xfrm rot='5400000' flipH='1'>",
    );
    let model = parse(&parts).unwrap();
    assert!(matches!(
        model.slide_contents()[0].shapes()[0],
        Shape::Transformed { .. }
    ));
    let ops = &model.to_display_list().pages[0].ops;
    assert!(matches!(ops[0], DrawOp::BeginTransform(_)));
    assert!(matches!(ops.last(), Some(DrawOp::EndTransform)));
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<p:spPr>",
        "<p:spPr><a:xfrm rot='0' flipH='0'/>",
    );
    let model = parse(&parts).unwrap();
    assert!(matches!(
        model.slide_contents()[0].shapes()[0],
        Shape::TextBox(_)
    ));
}

#[test]
fn strict_relationships_and_relocated_ancestors_resolve_without_path_guesses() {
    let mut parts = fixture("inherited-text-style.pptx");
    rewrite(
        &mut parts,
        "[Content_Types].xml",
        LAYOUT_PART,
        "templates/page.xml",
    );
    rewrite(
        &mut parts,
        "[Content_Types].xml",
        MASTER_PART,
        "templates/base.xml",
    );
    rewrite(
        &mut parts,
        "ppt/slides/_rels/slide1.xml.rels",
        "../slideLayouts/layout.xml",
        "../../templates/page.xml",
    );
    rewrite(
        &mut parts,
        "ppt/slideLayouts/_rels/layout.xml.rels",
        "../slideMasters/master.xml",
        "base.xml",
    );
    for (name, bytes) in &mut parts {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let s = String::from_utf8(bytes.clone()).unwrap();
            *bytes = s
                .replace(PML, "http://purl.oclc.org/ooxml/presentationml/main")
                .replace(DML, "http://purl.oclc.org/ooxml/drawingml/main")
                .replace(
                    OFFICE_RELS,
                    "http://purl.oclc.org/ooxml/officeDocument/relationships",
                )
                .into_bytes();
        }
        *name = match name.as_str() {
            LAYOUT_PART => "templates/page.xml".into(),
            MASTER_PART => "templates/base.xml".into(),
            "ppt/slideLayouts/_rels/layout.xml.rels" => "templates/_rels/page.xml.rels".into(),
            _ => name.clone(),
        };
    }
    let model = parse(&parts).unwrap();
    assert_eq!(
        model.slide_layouts()[0].master_part_name(),
        Some("templates/base.xml")
    );
    assert_eq!(text(&model, 0).rect.origin.x, 72.0);
    assert_eq!(text(&model, 0).paragraphs[0].runs[0].style.font_size, 28.0);
}

#[test]
fn reachable_ancestor_xml_obeys_byte_node_depth_limits() {
    for part in [LAYOUT_PART, MASTER_PART] {
        let mut parts = fixture("inherited-text-style.pptx");
        let bytes = &mut parts.iter_mut().find(|(n, _)| n == part).unwrap().1;
        let s = String::from_utf8(bytes.clone()).unwrap();
        *bytes = s
            .replace(
                "<p:cSld>",
                &format!(
                    "<p:cSld><p:extLst>{}{}",
                    "<p:ext>".repeat(64),
                    "</p:ext>".repeat(64)
                ),
            )
            .replace("<p:spTree>", "</p:extLst><p:spTree>")
            .into_bytes();
        assert!(matches!(
            parse(&parts),
            Err(PptxError::LimitExceeded {
                limit: "max_xml_depth"
            })
        ));
        let mut parts = fixture("inherited-text-style.pptx");
        let bytes = &mut parts.iter_mut().find(|(n, _)| n == part).unwrap().1;
        let s = String::from_utf8(bytes.clone()).unwrap();
        *bytes = s
            .replace("<p:cSld>", &format!("<p:cSld>{}", "<p:ext/>".repeat(1000)))
            .into_bytes();
        let package =
            OoxmlPackage::from_bytes(&zip(&parts, true, false), SecurityLimits::default()).unwrap();
        assert!(matches!(
            Presentation::parse_with_content(
                &package,
                ParseLimits {
                    max_xml_nodes: 500,
                    ..Default::default()
                }
            ),
            Err(PptxError::LimitExceeded {
                limit: "max_xml_nodes"
            })
        ));
        assert!(matches!(
            Presentation::parse_with_content(
                &package,
                ParseLimits {
                    max_xml_size: 2000,
                    ..Default::default()
                }
            ),
            Err(PptxError::LimitExceeded {
                limit: "max_xml_size"
            })
        ));
    }
}

#[test]
fn unknown_placeholder_identity_is_preserved_without_amplifying_diagnostics() {
    let kind = "x".repeat(10000);
    let local = placeholder(2, &kind, Some(1), FULL_GEOMETRY, None);
    let model = parse(&simple(&local, "", "")).unwrap();
    assert_eq!(model.slide_contents()[0].placeholders()[0].kind(), kind);
    assert!(model.warnings().iter().all(|w| w.to_string().len() < 500));
}
