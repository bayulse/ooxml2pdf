mod support;
use o2p_core::{Color, DrawOp};
use o2p_fonts::FontBook;
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{Fill, ParseLimits, PptxError, PptxWarning, Presentation, Shape, Stroke};
use std::path::PathBuf;
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    let package =
        OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default())
}
fn fixture(name: &str) -> Presentation {
    parse(
        &style_fixtures()
            .into_iter()
            .find(|(n, _)| *n == name)
            .unwrap()
            .1,
    )
    .unwrap()
}
fn color(hex: &str) -> Color {
    let value = u32::from_str_radix(hex, 16).unwrap();
    Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}
fn first_paint(model: &Presentation) -> (Option<Fill>, Option<Stroke>) {
    match model.slide_contents()[0].shapes()[0] {
        Shape::Rectangle { fill, stroke, .. } | Shape::Ellipse { fill, stroke, .. } => {
            (fill, stroke)
        }
        Shape::Line { stroke, .. } => (None, stroke),
        _ => panic!("expected primitive"),
    }
}
fn shape(fill: &str, line: &str, refs: &str) -> String {
    styled(&rectangle(2, NONTRIVIAL_RECT, fill, line), refs)
}
fn fonts() -> FontBook {
    let mut fonts = FontBook::new();
    fonts
        .register_directory(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
        .unwrap();
    fonts.set_fallback_families(vec!["O2P Test Sans".into(), "O2P Test Japanese".into()]);
    fonts
}
fn warning(model: &Presentation, needle: &str) {
    assert!(
        model
            .warnings()
            .iter()
            .any(|w| w.to_string().contains(needle)),
        "missing {needle}: {:?}",
        model.warnings()
    );
}
fn texts(model: &Presentation) -> Vec<&o2p_pptx::TextBox> {
    model.slide_contents()[0]
        .shapes()
        .iter()
        .filter_map(|s| match s {
            Shape::TextBox(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn fmt_scheme_is_owned_and_fixtures_are_reproducible_and_renderable() {
    for (name, parts) in style_fixtures() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(
            std::fs::read(path).unwrap(),
            zip(&parts, true, false),
            "{name}"
        );
        let model = parse(&parts).unwrap();
        let styles = &model.theme().unwrap().styles;
        assert_eq!(styles.fill_style_count(), 3);
        assert_eq!(styles.line_style_count(), 3);
        assert_eq!(styles.background_fill_style_count(), 3);
        assert_eq!(model.clone().theme(), model.theme());
        let report = model.to_display_list_with_fonts(&fonts()).unwrap();
        assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
        let bytes = o2p_pdf::render(&report.display_list).unwrap();
        assert_eq!(bytes, o2p_pdf::render(&report.display_list).unwrap());
        assert_eq!(
            lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(),
            1
        );
        if !matches!(
            name,
            "unsupported-gradient-style-ref.pptx" | "malformed-style-index.pptx"
        ) {
            assert!(
                model.warnings().iter().all(|w| matches!(w, PptxWarning::UnsupportedSlideContent { name, .. } if name.starts_with("legacy text box overflow:") || name.starts_with("text inheritance/font resolution:"))),
                "{name}: {:?}",
                model.warnings()
            );
        }
    }
}

#[test]
fn strict_namespaces_partial_styles_and_no_fill_without_theme_work() {
    let refs = format!(
        "{}{}",
        style_ref("fillRef", "1", "accent1"),
        style_ref("lnRef", "2", "accent4")
    );
    let mut parts = style_deck("", &shape("", "", &refs));
    for (_, bytes) in &mut parts {
        let text = String::from_utf8(bytes.clone())
            .unwrap()
            .replace(PML, "http://purl.oclc.org/ooxml/presentationml/main")
            .replace(DML, "http://purl.oclc.org/ooxml/drawingml/main")
            .replace(
                OFFICE_RELS,
                "http://purl.oclc.org/ooxml/officeDocument/relationships",
            );
        *bytes = text.into_bytes();
    }
    let model = parse(&parts).unwrap();
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("2060D0"))));
    assert_eq!(first_paint(&model).1.unwrap().width, 4.0);
    assert!(model.warnings().is_empty());
    let model = parse(&content_deck(
        "<p:bg><p:bgRef idx='1000'/></p:bg>",
        &shape("", "", "<a:fillRef idx='0'/><a:lnRef idx='0'/>"),
    ))
    .unwrap();
    assert_eq!(first_paint(&model), (Some(Fill::None), None));
    assert_eq!(model.slide_contents()[0].background(), Some(Fill::None));
    assert!(model.warnings().iter().all(is_missing_layout_warning));
    let model = parse(&style_deck("", &shape("", "", "<a:fillRef idx='2'/>"))).unwrap();
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("104830"))));
    assert!(model.warnings().is_empty());
    // DrawingML style containers use the same adapter as p:style.
    let model = parse(&style_deck(
        "",
        &shape("", "", &refs).replace("p:style", "a:style"),
    ))
    .unwrap();
    assert!(model.warnings().is_empty());
}

#[test]
fn source_scoped_theme_matrix_and_font_refs_survive_master_layout_traversal() {
    let refs = format!(
        "{}{}",
        style_ref("fillRef", "1", "accent1"),
        style_ref("fontRef", "minor", "accent4")
    );
    let local = placeholder(
        2,
        "body",
        Some(1),
        "",
        Some(&inherited_paragraph("Inherited font reference", "")),
    );
    let layout = placeholder(2, "body", Some(1), "", None);
    let master = styled(
        &placeholder(
            2,
            "body",
            None,
            &format!("{FULL_GEOMETRY}<a:prstGeom prst='rect'/>"),
            None,
        ),
        &refs,
    );
    let mut parts = attach_theme(
        inheritance_deck("", &local, "", &layout, "", &master),
        &style_theme().replace("2060D0", "AABBCC"),
    );
    replace(
        &mut parts,
        "ppt/_rels/presentation.xml.rels",
        rels(&relation("rId1", "slide", "slides/slide1.xml")),
    );
    parts.push((
        "ppt/slideMasters/_rels/master.xml.rels".into(),
        rels(&relation("rTheme", "theme", "../theme/synthetic.xml")),
    ));
    let model = parse(&parts).unwrap();
    assert!(model.theme().is_none());
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("AABBCC"))));
    assert_eq!(
        texts(&model)[0].paragraphs[0].runs[0]
            .style
            .family
            .as_deref(),
        Some("O2P Test Sans")
    );
    assert_eq!(
        model.slide_contents()[0].placeholders()[0]
            .text_defaults()
            .family
            .as_ref()
            .unwrap()
            .as_deref(),
        Some("O2P Test Sans")
    );
    assert!(model.warnings().iter().all(|w| matches!(w, PptxWarning::UnsupportedSlideContent { name, .. } if name.starts_with("text inheritance/font resolution:"))));
    assert_eq!(texts(&model)[0].paragraphs[0].runs[0].style.font_size, 18.0);
}

#[test]
fn unsupported_background_line_entries_and_effects_warn_even_when_overridden() {
    let theme = style_theme().replace(
        "<a:solidFill><a:schemeClr val='lt2'><a:tint val='50000'/></a:schemeClr></a:solidFill>",
        "<a:gradFill/>",
    );
    let model = parse(&attach_theme(
        inheritance_deck(
            &style_background("1001", "accent1"),
            "",
            &background("112233"),
            "",
            "",
            "",
        ),
        &theme,
    ))
    .unwrap();
    assert_eq!(model.slide_contents()[0].background(), None);
    warning(&model, "gradFill");
    let model = parse(&style_deck(
        "",
        &shape(
            "<a:noFill/>",
            "<a:ln xmlns:x='urn:unsupported'><x:noFill/></a:ln>",
            &style_ref("lnRef", "1", "accent1"),
        ),
    ))
    .unwrap();
    assert_eq!(first_paint(&model).1, None);
    warning(&model, "urn:unsupported");
    let theme = style_theme().replace(
        "<a:schemeClr val='lt2'><a:tint val='50000'/></a:schemeClr>",
        "<a:schemeClr val='lt2'><a:alpha val='50000'/></a:schemeClr>",
    );
    let model = parse(&attach_theme(
        inheritance_deck(
            &style_background("1001", "accent1"),
            "",
            &background("112233"),
            "",
            "",
            "",
        ),
        &theme,
    ))
    .unwrap();
    assert_eq!(model.slide_contents()[0].background(), None);
    warning(&model, "alpha");
    let theme = style_theme().replace("<a:ln w='25400'><a:solidFill><a:schemeClr val='phClr'/></a:solidFill><a:prstDash val='solid'/></a:ln>", "<a:unknownLine/>");
    let model = parse(&attach_theme(
        inheritance_deck(
            "",
            &shape("<a:noFill/>", "", &style_ref("lnRef", "2", "accent1")),
            "",
            "",
            "",
            "",
        ),
        &theme,
    ))
    .unwrap();
    assert_eq!(first_paint(&model).1.unwrap().width, 4.0);
    warning(&model, "unknownLine");
    let theme = style_theme().replace("<a:effectStyleLst/>", "<a:effectStyleLst><a:effectStyle><a:effectLst><a:innerShdw/></a:effectLst></a:effectStyle></a:effectStyleLst>");
    warning(
        &parse(&attach_theme(
            inheritance_deck("", "", "", "", "", ""),
            &theme,
        ))
        .unwrap(),
        "innerShdw",
    );
    let model = parse(&style_deck(
        "",
        &shape(
            &solid("112233"),
            &outline("445566", 12700),
            "<a:fillRef idx='bad'/><a:lnRef idx='99'/><a:effectRef idx='0'/>",
        ),
    ))
    .unwrap();
    assert_eq!(
        first_paint(&model),
        (
            Some(Fill::Solid(color("112233"))),
            Some(Stroke {
                color: color("445566"),
                width: 1.0
            })
        )
    );
    warning(&model, "fillRef");
    warning(&model, "lnRef");
    assert!(
        !model
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("effectRef"))
    );
}

#[test]
fn style_resolution_preserves_transform_scopes_image_resources_and_z_order() {
    let refs = format!(
        "{}{}",
        style_ref("fillRef", "1", "accent1"),
        style_ref("lnRef", "1", "accent4")
    );
    let styled_shape = shape("", "", &refs).replace("<a:xfrm>", "<a:xfrm rot='2700000' flipH='1'>");
    let mut parts = attach_theme(
        image_deck(
            false,
            &format!(
                "{}{}{}",
                styled_shape,
                picture(3, TEXT_RECT),
                shape("", "", &refs)
            ),
        ),
        &style_theme(),
    );
    // Supply explicit inheritance links without disturbing the image relationship.
    let base = inheritance_deck("", "", "", "", "", "");
    for (name, bytes) in base {
        if name.contains("slideLayouts/") || name.contains("slideMasters/") {
            parts.push((name, bytes));
        }
    }
    let types = parts
        .iter_mut()
        .find(|(n, _)| n == "[Content_Types].xml")
        .unwrap();
    types.1 = String::from_utf8(types.1.clone()).unwrap().replace("</Types>", &format!("<Override PartName='/{LAYOUT_PART}' ContentType='application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml'/><Override PartName='/{MASTER_PART}' ContentType='application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml'/></Types>")).into_bytes();
    let rel = parts
        .iter_mut()
        .find(|(n, _)| n == "ppt/slides/_rels/slide1.xml.rels")
        .unwrap();
    rel.1 = String::from_utf8(rel.1.clone())
        .unwrap()
        .replace(
            "</Relationships>",
            &format!(
                "{}</Relationships>",
                relation("rLayout", "slideLayout", "../slideLayouts/layout.xml")
            ),
        )
        .into_bytes();
    let model = parse(&parts).unwrap();
    let display = model.to_display_list();
    assert!(matches!(
        display.pages[0].ops.as_slice(),
        [
            DrawOp::Rect(_),
            DrawOp::BeginTransform(_),
            DrawOp::Rect(_),
            DrawOp::StrokeRect(_),
            DrawOp::EndTransform,
            DrawOp::Image(_),
            DrawOp::Rect(_),
            DrawOp::StrokeRect(_)
        ]
    ));
    let pdf = lopdf::Document::load_mem(&o2p_pdf::render(&display).unwrap()).unwrap();
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap();
    let ops: Vec<_> = content
        .operations
        .iter()
        .map(|o| o.operator.as_str())
        .filter(|o| matches!(*o, "f" | "S" | "Do"))
        .collect();
    assert_eq!(ops, ["f", "f", "S", "Do", "f", "S"]);
}

#[test]
fn fill_and_background_refs_share_split_one_based_index_space() {
    for (index, expected) in [
        ("0", Some(Fill::None)),
        ("1000", Some(Fill::None)),
        ("1", Some(Fill::Solid(color("2060D0")))),
        ("2", Some(Fill::Solid(color("104830")))),
        ("3", Some(Fill::None)),
        ("1001", Some(Fill::Solid(color("F4F6F9")))),
        ("1002", Some(Fill::Solid(color("2060D0")))),
        ("1003", Some(Fill::None)),
    ] {
        let model = parse(&style_deck(
            "",
            &shape("", "", &style_ref("fillRef", index, "accent1")),
        ))
        .unwrap();
        assert_eq!(first_paint(&model).0, expected, "fillRef {index}");
        let bg = parse(&style_deck(&style_background(index, "accent1"), "")).unwrap();
        assert_eq!(
            bg.slide_contents()[0].background(),
            expected,
            "bgRef {index}"
        );
        assert!(model.warnings().is_empty());
        assert!(bg.warnings().is_empty());
    }
}

#[test]
fn line_refs_use_one_based_lines_and_zero_means_no_paint() {
    for (index, expected) in [
        ("0", None),
        (
            "1",
            Some(Stroke {
                color: color("C43D56"),
                width: 2.0,
            }),
        ),
        (
            "2",
            Some(Stroke {
                color: color("8060B0"),
                width: 4.0,
            }),
        ),
        ("3", None),
    ] {
        let model = parse(&style_deck(
            "",
            &shape("<a:noFill/>", "", &style_ref("lnRef", index, "accent4")),
        ))
        .unwrap();
        assert_eq!(first_paint(&model).1, expected, "lnRef {index}");
        assert!(model.warnings().is_empty());
    }
    let model = parse(&style_deck(
        "",
        &shape("<a:noFill/>", "", &style_ref("lnRef", "1001", "accent4")),
    ))
    .unwrap();
    assert_eq!(first_paint(&model).1, None);
    warning(&model, "lnRef idx 1001");
}

#[test]
fn rectangles_ellipses_lines_and_text_shapes_use_existing_display_ops() {
    let refs = format!(
        "{}{}",
        style_ref("fillRef", "1", "accent1"),
        style_ref("lnRef", "1", "accent4")
    );
    let shapes = format!(
        "{}{}{}{}",
        shape("", "", &refs),
        styled(&ellipse(3, NONTRIVIAL_RECT, "", ""), &refs),
        straight_line(4, (0, 0, 1_270_000, 0), "").replace(
            "</p:cxnSp>",
            &format!("<p:style>{refs}</p:style></p:cxnSp>")
        ),
        styled(
            &text_shape(
                5,
                TEXT_RECT,
                &inherited_paragraph("Styled text", ""),
                "",
                ""
            ),
            &refs
        )
    );
    let model = parse(&style_deck("", &shapes)).unwrap();
    assert!(model.warnings().iter().all(|w| matches!(w, PptxWarning::UnsupportedSlideContent { name, .. } if name.starts_with("text inheritance/font resolution:"))), "{:?}", model.warnings());
    assert!(matches!(
        model.to_display_list().pages[0].ops.as_slice(),
        [
            DrawOp::Rect(_),
            DrawOp::StrokeRect(_),
            DrawOp::Ellipse(_),
            DrawOp::StrokeEllipse(_),
            DrawOp::Line(_),
            DrawOp::Rect(_),
            DrawOp::StrokeRect(_),
            DrawOp::Text(_)
        ]
    ));
    let text_only = styled(
        &text_shape(
            2,
            TEXT_RECT,
            &inherited_paragraph("Text-only paint", ""),
            "",
            "",
        )
        .replace("<a:prstGeom prst='rect'><a:avLst/></a:prstGeom>", ""),
        &refs,
    );
    let model = parse(&style_deck("", &text_only)).unwrap();
    assert!(matches!(
        model.to_display_list().pages[0].ops.as_slice(),
        [DrawOp::Rect(_), DrawOp::StrokeRect(_), DrawOp::Text(_)]
    ));
}

#[test]
fn direct_paint_overrides_refs_per_property_including_no_fill_and_zero_width() {
    assert_eq!(
        first_paint(&fixture("style-local-fill-overrides-ref.pptx")).0,
        Some(Fill::Solid(color("E88A20")))
    );
    assert_eq!(
        first_paint(&fixture("style-local-line-overrides-ref.pptx")).1,
        Some(Stroke {
            color: color("24334A"),
            width: 5.0
        })
    );
    let refs = format!(
        "{}{}",
        style_ref("fillRef", "1", "accent1"),
        style_ref("lnRef", "1", "accent4")
    );
    for (line, expected) in [
        (
            "<a:ln w='63500'/>",
            Some(Stroke {
                color: color("C43D56"),
                width: 5.0,
            }),
        ),
        (
            "<a:ln><a:solidFill><a:srgbClr val='112233'/></a:solidFill></a:ln>",
            Some(Stroke {
                color: color("112233"),
                width: 2.0,
            }),
        ),
        ("<a:ln><a:noFill/></a:ln>", None),
        ("<a:ln w='0'/>", None),
    ] {
        let model = parse(&style_deck("", &shape("<a:noFill/>", line, &refs))).unwrap();
        assert_eq!(first_paint(&model), (Some(Fill::None), expected));
    }
    let model = parse(&style_deck(
        "",
        &shape(
            "<a:noFill/>",
            "<a:ln><a:solidFill><a:srgbClr val='112233'/></a:solidFill></a:ln>",
            &style_ref("lnRef", "0", "accent4"),
        ),
    ))
    .unwrap();
    assert_eq!(first_paint(&model).1.unwrap().width, 0.75);
    let model = parse(&style_deck(
        "",
        &shape("<a:gradFill/>", "<a:ln><a:gradFill/></a:ln>", &refs),
    ))
    .unwrap();
    assert_eq!(first_paint(&model), (None, None));
    warning(&model, "gradFill");
}

#[test]
fn master_layout_refs_inherit_and_decorations_do_not_duplicate_prompts() {
    for name in ["master-style-ref.pptx", "layout-style-ref.pptx"] {
        let model = fixture(name);
        assert_eq!(model.slide_contents()[0].placeholders().len(), 1);
        assert_eq!(model.slide_contents()[0].shapes().len(), 3);
        assert!(
            matches!(model.slide_contents()[0].shapes()[0], Shape::Ellipse { fill: Some(Fill::Solid(c)), .. } if c == color("2060D0"))
        );
        assert!(matches!(
            model.to_display_list().pages[0].ops.as_slice(),
            [
                DrawOp::Ellipse(_),
                DrawOp::StrokeEllipse(_),
                DrawOp::Rect(_),
                DrawOp::StrokeRect(_),
                DrawOp::Text(_)
            ]
        ));
        assert_eq!(
            texts(&model)[0].paragraphs[0].runs[0].text,
            "Inherited style — Résumé"
        );
    }
}

#[test]
fn inherited_direct_formatting_beats_local_refs_and_refs_merge_separately() {
    let refs = format!(
        "{}{}{}",
        style_ref("fillRef", "1", "accent1"),
        style_ref("lnRef", "2", "accent1"),
        style_ref("fontRef", "major", "accent1")
    );
    let local = styled(
        &placeholder(
            2,
            "body",
            Some(1),
            "",
            Some(&inherited_paragraph("Local content", "")),
        ),
        &refs,
    );
    let master = placeholder(
        2,
        "body",
        None,
        &format!(
            "{FULL_GEOMETRY}<a:prstGeom prst='rect'/>{}{}",
            solid("FFCC88"),
            outline("112233", 38100)
        ),
        Some(&inherited_paragraph(
            "Never rendered",
            "<a:pPr><a:defRPr><a:latin typeface='Inherited Family'/><a:solidFill><a:srgbClr val='445566'/></a:solidFill></a:defRPr></a:pPr>",
        )),
    );
    let layout = styled(
        &placeholder(2, "body", Some(1), "", None),
        &format!(
            "{}{}",
            style_ref("fillRef", "2", "accent2"),
            style_ref("lnRef", "1", "accent3")
        ),
    );
    let model = parse(&attach_theme(
        inheritance_deck("", &local, "", &layout, "", &master),
        &style_theme(),
    ))
    .unwrap();
    assert_eq!(
        first_paint(&model),
        (
            Some(Fill::Solid(color("FFCC88"))),
            Some(Stroke {
                color: color("112233"),
                width: 3.0
            })
        )
    );
    let text = &texts(&model)[0].paragraphs[0].runs[0].style;
    assert_eq!(text.family.as_deref(), Some("Inherited Family"));
    assert_eq!(text.color, color("445566"));
    let bare_master = styled(
        &placeholder(
            2,
            "body",
            None,
            &format!("{FULL_GEOMETRY}<a:prstGeom prst='rect'/>"),
            None,
        ),
        &style_ref("fillRef", "2", "accent2"),
    );
    let model = parse(&attach_theme(
        inheritance_deck("", &local, "", &layout, "", &bare_master),
        &style_theme(),
    ))
    .unwrap();
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("2060D0"))));
    assert_eq!(first_paint(&model).1.unwrap().width, 4.0);
    let local = local.replace(
        "<p:spPr>",
        &format!("<p:spPr>{}<a:ln w='12700'/>", solid("AABBCC")),
    );
    let model = parse(&attach_theme(
        inheritance_deck("", &local, "", &layout, "", &master),
        &style_theme(),
    ))
    .unwrap();
    assert_eq!(
        first_paint(&model),
        (
            Some(Fill::Solid(color("AABBCC"))),
            Some(Stroke {
                color: color("112233"),
                width: 1.0
            })
        )
    );
}

#[test]
fn background_presence_keeps_slide_layout_master_priority() {
    let master = style_background("1002", "accent4");
    let layout = style_background("1002", "accent2");
    let slide = style_background("1002", "accent1");
    for (slide, layout, expected) in [
        ("", "", Some(Fill::Solid(color("C43D56")))),
        ("", layout.as_str(), Some(Fill::Solid(color("209060")))),
        (
            slide.as_str(),
            layout.as_str(),
            Some(Fill::Solid(color("2060D0"))),
        ),
        (
            "<p:bg><p:bgRef idx='0'/></p:bg>",
            layout.as_str(),
            Some(Fill::None),
        ),
        (
            "<p:bg><p:bgRef idx='1000'/></p:bg>",
            layout.as_str(),
            Some(Fill::None),
        ),
        ("<p:bg><p:bgRef idx='5000'/></p:bg>", layout.as_str(), None),
    ] {
        let model = parse(&attach_theme(
            inheritance_deck(slide, "", layout, "", &master, ""),
            &style_theme(),
        ))
        .unwrap();
        assert_eq!(model.slide_contents()[0].background(), expected);
    }
    let background = format!(
        "<p:bg><p:bgPr>{}</p:bgPr><p:bgRef idx='1002'><a:schemeClr val='accent1'/></p:bgRef></p:bg>",
        solid("112233")
    );
    assert_eq!(
        parse(&style_deck(&background, ""))
            .unwrap()
            .slide_contents()[0]
            .background(),
        Some(Fill::Solid(color("112233")))
    );
}

#[test]
fn reference_colors_and_matrix_transforms_share_central_resolver() {
    let theme = style_theme().replace(
        "<a:schemeClr val='phClr'/>",
        "<a:schemeClr val='phClr'><a:tint val='50000'/></a:schemeClr>",
    );
    let refs = "<a:fillRef idx='1'><a:srgbClr val='FF0000'><a:shade val='50000'/></a:srgbClr></a:fillRef><a:lnRef idx='1'><a:srgbClr val='FF0000'><a:shade val='50000'/></a:srgbClr></a:lnRef>";
    let model = parse(&attach_theme(
        inheritance_deck("", &shape("", "", refs), "", "", "", ""),
        &theme,
    ))
    .unwrap();
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("C08080"))));
    assert_eq!(first_paint(&model).1.unwrap().color, color("C08080"));
    let theme = theme.replace(
        "<a:schemeClr val='phClr'><a:tint val='50000'/></a:schemeClr>",
        "<a:schemeClr val='accent3'><a:lumMod val='50000'/><a:lumOff val='25000'/></a:schemeClr>",
    ).replace("E88A20", "FF0000");
    let model = parse(&attach_theme(
        inheritance_deck("", &shape("", "", refs), "", "", "", ""),
        &theme,
    ))
    .unwrap();
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("FF0000"))));
    assert_eq!(first_paint(&model).1.unwrap().color, color("FF0000"));
    assert!(model.warnings().is_empty());
    let model = parse(&style_deck(
        "",
        &shape("", "", "<a:fillRef idx='1'/><a:lnRef idx='2'/>"),
    ))
    .unwrap();
    assert_eq!(first_paint(&model).0, None);
    assert_eq!(first_paint(&model).1.unwrap().color, color("8060B0"));
    warning(&model, "phClr");
}

#[test]
fn font_refs_resolve_roles_and_keep_paragraph_run_priority_and_unicode() {
    let theme = style_theme().replace(
        "<a:minorFont><a:latin typeface='O2P Test Sans'",
        "<a:minorFont><a:latin typeface='Minor Family'",
    );
    for (role, expected) in [
        ("major", Some("O2P Test Sans")),
        ("minor", Some("Minor Family")),
        ("none", None),
    ] {
        let base = text_shape(
            2,
            TEXT_RECT,
            &inherited_paragraph("Résumé", ""),
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        );
        let model = parse(&attach_theme(
            inheritance_deck(
                "",
                &styled(&base, &style_ref("fontRef", role, "accent4")),
                "",
                "",
                "",
                "",
            ),
            &theme,
        ))
        .unwrap();
        let style = &texts(&model)[0].paragraphs[0].runs[0].style;
        assert_eq!(style.family.as_deref(), expected);
        assert_eq!(style.color, color("C43D56"));
    }
    let paragraphs = "<a:p><a:pPr><a:defRPr><a:latin typeface='Paragraph Family'/><a:solidFill><a:srgbClr val='112233'/></a:solidFill></a:defRPr></a:pPr><a:r><a:rPr><a:latin typeface='Run Family'/><a:solidFill><a:srgbClr val='AABBCC'/></a:solidFill></a:rPr><a:t>Run</a:t></a:r><a:r><a:t>Paragraph</a:t></a:r></a:p>";
    let model = parse(&style_deck(
        "",
        &styled(
            &text_shape(
                2,
                TEXT_RECT,
                paragraphs,
                "<a:noFill/>",
                "<a:ln><a:noFill/></a:ln>",
            ),
            &style_ref("fontRef", "major", "accent4"),
        ),
    ))
    .unwrap();
    let runs = &texts(&model)[0].paragraphs[0].runs;
    assert_eq!(runs[0].style.family.as_deref(), Some("Run Family"));
    assert_eq!(runs[0].style.color, color("AABBCC"));
    assert_eq!(runs[1].style.family.as_deref(), Some("Paragraph Family"));
    assert_eq!(runs[1].style.color, color("112233"));
    let model = fixture("theme-font-ref.pptx");
    let report = model.to_display_list_with_fonts(&fonts()).unwrap();
    let bytes = o2p_pdf::render(&report.display_list).unwrap();
    assert!(bytes.windows(10).any(|w| w == b"/FontFile2"));
    assert!(bytes.windows(10).any(|w| w == b"/ToUnicode"));
    assert!(
        lopdf::Document::load_mem(&bytes)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains("Résumé")
    );
}

#[test]
fn malformed_missing_and_out_of_range_refs_warn_without_panics() {
    for name in ["fillRef", "lnRef"] {
        for index in [
            "invalid",
            "-1",
            "1.5",
            "4294967296",
            "4",
            "999",
            "1004",
            "4294967295",
        ] {
            let model = parse(&style_deck(
                "",
                &shape("", "", &style_ref(name, index, "accent1")),
            ))
            .unwrap();
            assert_eq!(first_paint(&model), (None, None));
            warning(&model, name);
            assert!(model.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedSlideContent { slide_id: 256, part, name: n } if part == "ppt/slides/slide1.xml" && n.contains(name))));
        }
    }
    for name in ["fillRef", "lnRef", "fontRef"] {
        warning(
            &parse(&style_deck("", &shape("", "", &format!("<a:{name}/>")))).unwrap(),
            name,
        );
    }
    warning(
        &parse(&content_deck(
            "",
            &shape("", "", &style_ref("fillRef", "1", "accent1")),
        ))
        .unwrap(),
        "style fillRef",
    );
    let model = parse(&attach_theme(
        content_deck("", &shape("", "", &style_ref("fillRef", "1", "accent1"))),
        &theme_xml(),
    ))
    .unwrap();
    assert_eq!(first_paint(&model).0, None);
    warning(&model, "out of range");
}

#[test]
fn unsupported_styles_warn_keep_positions_and_never_approximate() {
    let gradient = fixture("unsupported-gradient-style-ref.pptx");
    assert_eq!(first_paint(&gradient).0, None);
    warning(&gradient, "gradFill");
    assert!(gradient.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedTheme { part, name } if part == THEME_PART && name.contains("gradFill"))));
    assert!(gradient.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedSlideContent { name, .. } if name.contains("style-matrix") && name.contains("gradFill"))));
    for unsupported in [
        "<a:gradFill/>",
        "<a:pattFill/>",
        "<a:blipFill/>",
        "<a:grpFill/>",
        "<a:unknownFill/>",
    ] {
        let theme = style_theme().replacen(
            "<a:solidFill><a:schemeClr val='phClr'/></a:solidFill>",
            unsupported,
            1,
        );
        for (index, expected) in [("1", None), ("2", Some(Fill::Solid(color("104830"))))] {
            let model = parse(&attach_theme(
                inheritance_deck(
                    "",
                    &shape("", "", &style_ref("fillRef", index, "accent1")),
                    "",
                    "",
                    "",
                    "",
                ),
                &theme,
            ))
            .unwrap();
            assert_eq!(first_paint(&model).0, expected);
            warning(&model, "unsupported fill");
        }
    }
    for decoration in [
        "<a:headEnd type='triangle'/>",
        "<a:prstDash val='dash'/>",
        "<a:round/>",
        "<a:miter lim='800000'/>",
    ] {
        let theme = style_theme().replace("<a:prstDash val='solid'/>", decoration);
        let model = parse(&attach_theme(
            inheritance_deck(
                "",
                &shape("<a:noFill/>", "", &style_ref("lnRef", "1", "accent1")),
                "",
                "",
                "",
                "",
            ),
            &theme,
        ))
        .unwrap();
        assert_eq!(first_paint(&model).1, None);
        warning(&model, "stroke omitted");
    }
    let model = parse(&style_deck(
        "",
        &shape(
            &solid("AABBCC"),
            &outline("112233", 12700),
            "<a:effectRef idx='1'/><a:unknownRef/>",
        ),
    ))
    .unwrap();
    warning(&model, "effectRef");
    warning(&model, "unknownRef");
    assert_eq!(first_paint(&model).0, Some(Fill::Solid(color("AABBCC"))));
}

#[test]
fn malformed_matrix_and_security_limits_use_existing_boundaries() {
    for (from, to) in [
        ("w='25400'", "w='-1'"),
        ("w='25400'", "w='9999999999999'"),
        ("<a:fillStyleLst>", "<a:fillStyleLst>text"),
        (
            "<a:schemeClr val='phClr'/>",
            "<a:schemeClr val='phClr'/><a:srgbClr val='112233'/>",
        ),
        (
            "<a:schemeClr val='phClr'/>",
            "<a:schemeClr val='phClr'><a:tint val='100001'/></a:schemeClr>",
        ),
        ("</a:fmtScheme>", "</a:fmtScheme><a:fmtScheme/>"),
        ("</a:fillStyleLst>", "</a:fillStyleLst><a:fillStyleLst/>"),
    ] {
        assert!(
            matches!(
                parse(&attach_theme(
                    inheritance_deck("", "", "", "", "", ""),
                    &style_theme().replace(from, to)
                )),
                Err(PptxError::InvalidTheme { .. })
            ),
            "{to}"
        );
    }
    assert!(matches!(
        parse(&style_deck(
            "",
            &shape("", "", "<a:fillRef idx='1'/><a:fillRef idx='2'/>")
        )),
        Err(PptxError::InvalidSlide { .. })
    ));
    let parts = attach_theme(
        inheritance_deck("", "", "", "", "", ""),
        &style_theme().replace(
            "<a:schemeClr val='phClr'/>",
            "<a:schemeClr val='phClr'><a:alpha val='50000'/></a:schemeClr>",
        ),
    );
    warning(&parse(&parts).unwrap(), "alpha");
    let package =
        OoxmlPackage::from_bytes(&zip(&parts, true, false), SecurityLimits::default()).unwrap();
    assert!(matches!(
        Presentation::parse_with_content(
            &package,
            ParseLimits {
                max_xml_nodes: 40,
                ..Default::default()
            }
        ),
        Err(PptxError::LimitExceeded { .. })
    ));
    let bad = attach_theme(
        inheritance_deck("", "", "", "", "", ""),
        &style_theme().replace("w='25400'", "w='bad'"),
    );
    let package =
        OoxmlPackage::from_bytes(&zip(&bad, true, false), SecurityLimits::default()).unwrap();
    assert!(Presentation::parse(&package, ParseLimits::default()).is_ok());
}

#[test]
fn demo_preserves_z_order_wrapping_fonts_unicode_and_local_override() {
    let model = fixture("style-matrix-demo.pptx");
    let report = model.to_display_list_with_fonts(&fonts()).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let ops = &report.display_list.pages[0].ops;
    assert!(matches!(ops[0], DrawOp::Rect(ref r) if r.color == color("F4F6F9")));
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Rect(r) if r.color == color("2060D0")))
    );
    assert!(ops.iter().any(
        |op| matches!(op, DrawOp::StrokeRect(r) if r.color == color("C43D56") && r.width == 2.0)
    ));
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Rect(r) if r.color == color("E88A20")))
    );
    assert!(
        ops.iter()
            .filter(|op| matches!(op, DrawOp::GlyphText(_)))
            .count()
            > texts(&model).len()
    );
    let bytes = o2p_pdf::render(&report.display_list).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    for label in [
        "DrawingML style matrix — Résumé",
        "Theme fill",
        "Theme line",
        "Local override",
        "selectable Unicode",
    ] {
        assert!(extracted.contains(label), "{label}: {extracted}");
    }
}
