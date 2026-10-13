mod support;
use o2p_core::Color;
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{
    Fill, ParseLimits, PptxError, PptxWarning, Presentation, Shape, TextAlignment, TextBox,
    ThemeColorSlot,
};
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    Presentation::parse_with_content(
        &OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default())?,
        ParseLimits::default(),
    )
}
fn fixture(name: &str) -> Parts {
    fidelity2a_fixtures()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
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
fn warning(model: &Presentation, needle: &str) -> bool {
    model.warnings().iter().any(
        |w| matches!(w, PptxWarning::UnsupportedSlideContent { name, .. } if name.contains(needle)),
    )
}
fn rewrite(parts: &mut Parts, part: &str, from: &str, to: &str) {
    let bytes = &mut parts.iter_mut().find(|(n, _)| n == part).unwrap().1;
    let xml = String::from_utf8(bytes.clone()).unwrap();
    assert!(xml.contains(from), "{part}: {from}");
    *bytes = xml.replace(from, to).into_bytes();
}
fn rgb(name: &str) -> Color {
    let value = THEME_COLORS.iter().find(|(n, _)| *n == name).unwrap().1;
    let n = u32::from_str_radix(value, 16).unwrap();
    Color::rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}

#[test]
fn title_body_and_other_master_styles_resolve_individually_without_fallback() {
    for (name, role) in [
        ("master-title-style.pptx", "title"),
        ("master-body-style.pptx", "body"),
        ("master-other-style.pptx", "other"),
    ] {
        let model = parse(&fixture(name)).unwrap();
        let paragraph = &text(&model).paragraphs[0];
        let style = &paragraph.runs[0].style;
        assert_eq!(style.font_size, 32.0, "{name}");
        assert_eq!(style.family.as_deref(), Some("O2P Test Sans"));
        assert_eq!(style.color, rgb("accent2"));
        assert!(style.bold && style.italic);
        assert_eq!(paragraph.alignment, TextAlignment::Center);
        assert!(
            !warning(&model, "text inheritance/font resolution"),
            "{:?}",
            model.warnings()
        );
        assert!(!warning(&model, "txStyles"));
        let styles = model.slide_masters()[0].text_styles();
        let list = match role {
            "title" => &styles.title,
            "body" => &styles.body,
            _ => &styles.other,
        };
        assert_eq!(list.level(0).unwrap().font_size, Some(32.0));
        assert!(list.level(9).is_none());
        if let Some(placeholder) = model.slide_contents()[0].placeholders().first() {
            assert_eq!(placeholder.text_defaults().font_size, Some(32.0));
        }
    }
}

#[test]
fn paragraph_level_selects_its_own_master_style_and_does_not_reset_local_properties() {
    let model = parse(&fixture("paragraph-level-style.pptx")).unwrap();
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 21.0);
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        rgb("accent3")
    );
    assert_eq!(text(&model).paragraphs[0].alignment, TextAlignment::Right);
    assert!(!text(&model).paragraphs[0].runs[0].style.bold);
    assert!(!text(&model).paragraphs[0].runs[0].style.italic);
    assert!(!warning(&model, "paragraph level"));
    assert!(!warning(&model, "text inheritance/font resolution"));
}

#[test]
fn focused_inheritance_fixtures_cover_distinct_property_sources() {
    let size = parse(&fixture("inherited-font-size.pptx")).unwrap();
    assert_eq!(text(&size).paragraphs[0].runs[0].style.font_size, 26.0);
    let family = parse(&fixture("inherited-font-family.pptx")).unwrap();
    assert_eq!(
        text(&family).paragraphs[0].runs[0].style.family.as_deref(),
        Some("O2P Test Japanese")
    );
    let flags = parse(&fixture("inherited-bold-italic.pptx")).unwrap();
    assert!(!text(&flags).paragraphs[0].runs[0].style.bold);
    assert!(text(&flags).paragraphs[0].runs[0].style.italic);
    let color = parse(&fixture("inherited-text-color.pptx")).unwrap();
    assert_eq!(
        text(&color).paragraphs[0].runs[0].style.color,
        rgb("accent3")
    );
    for model in [&size, &family, &flags, &color] {
        assert!(!warning(model, "text inheritance/font resolution"));
    }
}

#[test]
fn subtitle_uses_body_style_while_footer_roles_use_other_style() {
    let styles = format!(
        "{}<p:otherStyle><a:lvl1pPr><a:defRPr sz='1500'><a:latin typeface='+mn-lt'/><a:solidFill><a:schemeClr val='accent3'/></a:solidFill></a:defRPr></a:lvl1pPr></p:otherStyle>",
        master_style("bodyStyle")
    );
    let subtitle = parse(&text_style_deck("subTitle", &styles, "", "", "")).unwrap();
    assert_eq!(text(&subtitle).paragraphs[0].runs[0].style.font_size, 32.0);
    assert_eq!(
        text(&subtitle).paragraphs[0].runs[0].style.color,
        rgb("accent2")
    );
    for kind in ["ftr", "sldNum", "dt"] {
        let model = parse(&text_style_deck(kind, &styles, "", "", "")).unwrap();
        assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 15.0);
        assert_eq!(
            text(&model).paragraphs[0].runs[0].style.color,
            rgb("accent3")
        );
    }
}

#[test]
fn all_nine_levels_and_def_ppr_resolve_without_borrowing_level_one() {
    let levels: String = (1..=9)
        .map(|i| {
            format!(
                "<a:lvl{i}pPr><a:defRPr sz='{}'/></a:lvl{i}pPr>",
                1000 + 100 * i
            )
        })
        .collect();
    let styles = format!("<p:bodyStyle><a:defPPr>{FULL_TEXT}</a:defPPr>{levels}</p:bodyStyle>");
    for i in 0..9 {
        let model = parse(&text_style_deck(
            "body",
            &styles,
            &format!("<a:pPr lvl='{i}'/>"),
            "",
            "",
        ))
        .unwrap();
        assert_eq!(
            text(&model).paragraphs[0].runs[0].style.font_size,
            11.0 + i as f32
        );
        assert!(text(&model).paragraphs[0].runs[0].style.bold);
        assert!(!warning(&model, "text inheritance/font resolution"));
    }
}

#[test]
fn slide_layout_master_and_text_style_properties_have_independent_precedence() {
    let styles = master_style("bodyStyle");
    let local = "<a:pPr><a:defRPr sz='1900' i='0'/></a:pPr>";
    let layout = "<a:pPr algn='r'><a:defRPr sz='2400' b='0'/></a:pPr>";
    let master = "<a:pPr><a:defRPr sz='2800'><a:latin typeface='Master Family'/><a:solidFill><a:srgbClr val='123456'/></a:solidFill></a:defRPr></a:pPr>";
    let model = parse(&text_style_deck("body", &styles, local, layout, master)).unwrap();
    let paragraph = &text(&model).paragraphs[0];
    let style = &paragraph.runs[0].style;
    assert_eq!(style.font_size, 19.0);
    assert_eq!(style.family.as_deref(), Some("Master Family"));
    assert_eq!(style.color, Color::rgb(0x12, 0x34, 0x56));
    assert!(!style.bold && !style.italic);
    assert_eq!(paragraph.alignment, TextAlignment::Right);
    assert!(!warning(&model, "text inheritance/font resolution"));
    let layout_only = parse(&text_style_deck("body", &styles, "", layout, master)).unwrap();
    assert_eq!(
        text(&layout_only).paragraphs[0].runs[0].style.font_size,
        24.0
    );
    assert!(text(&layout_only).paragraphs[0].runs[0].style.italic);
    let master_only = parse(&text_style_deck("body", &styles, "", "", master)).unwrap();
    assert_eq!(
        text(&master_only).paragraphs[0].runs[0].style.font_size,
        28.0
    );
}

#[test]
fn local_run_overrides_paragraph_list_placeholder_and_master_style() {
    let mut parts = fixture("local-text-override.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:p>",
        "<a:p><a:pPr algn='r'><a:defRPr sz='2300' b='1' i='1'/></a:pPr>",
    );
    let model = parse(&parts).unwrap();
    let p = &text(&model).paragraphs[0];
    assert_eq!(p.runs[0].style.font_size, 17.0);
    assert_eq!(p.runs[0].style.family.as_deref(), Some("Local Font"));
    assert_eq!(p.runs[0].style.color, Color::rgb(0xAA, 0xBB, 0xCC));
    assert!(!p.runs[0].style.bold && !p.runs[0].style.italic);
    assert_eq!(p.alignment, TextAlignment::Right);
}

#[test]
fn shape_local_list_defaults_override_ancestor_levels_and_apply_to_ordinary_text() {
    let mut parts = fixture("master-body-style.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:lstStyle/>",
        "<a:lstStyle><a:defPPr><a:defRPr sz='1800' b='0'/></a:defPPr><a:lvl1pPr algn='l'><a:defRPr sz='2200'/></a:lvl1pPr></a:lstStyle>",
    );
    let model = parse(&parts).unwrap();
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 22.0);
    assert!(!text(&model).paragraphs[0].runs[0].style.bold);
    assert!(text(&model).paragraphs[0].runs[0].style.italic);
    assert_eq!(text(&model).paragraphs[0].alignment, TextAlignment::Left);
    // Non-placeholder text must use otherStyle even without a shape style ref.
    let ordinary = text_shape(3, TEXT_RECT, &inherited_paragraph("Ordinary", ""), "", "");
    let mut parts = text_style_deck("body", &master_style("otherStyle"), "", "", "");
    replace(&mut parts, "ppt/slides/slide1.xml", slide("", &ordinary));
    let model = parse(&parts).unwrap();
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 32.0);
    assert!(!warning(&model, "text inheritance/font resolution"));
}

#[test]
fn layout_and_master_paragraph_defaults_are_selected_by_level() {
    let mut parts = text_style_deck(
        "body",
        &format!("<p:bodyStyle><a:defPPr>{FULL_TEXT}</a:defPPr></p:bodyStyle>"),
        "<a:pPr lvl='2'/>",
        "<a:pPr lvl='2'><a:defRPr sz='2700' b='0'/></a:pPr>",
        "<a:pPr lvl='2' algn='r'><a:defRPr sz='3000' i='0'/></a:pPr>",
    );
    let model = parse(&parts).unwrap();
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 27.0);
    assert!(!text(&model).paragraphs[0].runs[0].style.bold);
    assert!(!text(&model).paragraphs[0].runs[0].style.italic);
    assert_eq!(text(&model).paragraphs[0].alignment, TextAlignment::Right);
    rewrite(
        &mut parts,
        LAYOUT_PART,
        "<a:lstStyle/>",
        "<a:lstStyle><a:lvl3pPr><a:defRPr sz='2500'/></a:lvl3pPr></a:lstStyle>",
    );
    assert_eq!(
        text(&parse(&parts).unwrap()).paragraphs[0].runs[0]
            .style
            .font_size,
        27.0
    );
}

#[test]
fn insets_inherit_individually_with_zero_and_local_values_preserved() {
    let mut parts = fixture("master-body-style.pptx");
    rewrite(
        &mut parts,
        MASTER_PART,
        "<a:bodyPr/>",
        "<a:bodyPr lIns='127000' rIns='254000' tIns='38100' bIns='50800'/>",
    );
    rewrite(
        &mut parts,
        LAYOUT_PART,
        "<a:bodyPr/>",
        "<a:bodyPr lIns='0' bIns='127000'/>",
    );
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:bodyPr/>",
        "<a:bodyPr rIns='381000'/>",
    );
    let model = parse(&parts).unwrap();
    let i = text(&model).insets;
    assert_eq!((i.left, i.right, i.top, i.bottom), (0.0, 30.0, 3.0, 10.0));
    assert!(!warning(&model, "bodyPr"));
}

#[test]
fn unresolved_hierarchy_warns_and_local_direct_formatting_can_resolve_it() {
    let mut parts = text_style_deck("body", "", "", "", "");
    let model = parse(&parts).unwrap();
    assert!(warning(&model, "text inheritance/font resolution"));
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 18.0);
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:r>",
        &format!("<a:r>{}", FULL_TEXT.replace("defRPr", "rPr")),
    );
    assert!(!warning(
        &parse(&parts).unwrap(),
        "text inheritance/font resolution"
    ));
    // A missing theme font is still diagnosed despite resolved size/color.
    rewrite(
        &mut parts,
        THEME_PART,
        "typeface='O2P Test Sans'",
        "typeface=''",
    );
    let model = parse(&parts).unwrap();
    assert!(warning(&model, "font"));
    assert!(warning(&model, "text inheritance/font resolution"));
}

#[test]
fn malformed_paragraph_levels_and_duplicate_level_styles_return_errors() {
    let mut parts = fixture("paragraph-level-style.pptx");
    rewrite(&mut parts, "ppt/slides/slide1.xml", "lvl='2'", "lvl='9'");
    assert!(matches!(parse(&parts), Err(PptxError::InvalidSlide { .. })));
    let mut parts = fixture("master-body-style.pptx");
    rewrite(
        &mut parts,
        MASTER_PART,
        "</p:bodyStyle>",
        "<a:lvl1pPr/></p:bodyStyle>",
    );
    assert!(matches!(parse(&parts), Err(PptxError::InvalidSlide { .. })));
}

#[test]
fn master_map_is_owned_and_keeps_theme_rgb_definitions_unchanged() {
    let model = parse(&fixture("master-clrmap.pptx")).unwrap();
    let map = model.slide_masters()[0].color_map();
    assert_eq!(map.resolve_slot("tx1"), Some(ThemeColorSlot::Accent2));
    assert_eq!(map.resolve_slot("accent1"), Some(ThemeColorSlot::Accent3));
    for token in [
        "bg1", "bg2", "tx2", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink",
        "folHlink",
    ] {
        assert!(map.resolve_slot(token).is_some(), "{token}");
    }
    assert_eq!(map.resolve_slot("dk1"), Some(ThemeColorSlot::Dark1));
    assert_eq!(
        model.theme().unwrap().resolve_color("accent1"),
        Some(rgb("accent1"))
    );
    assert_eq!(model.theme().unwrap().resolve_color("tx1"), None);
    assert!(!warning(&model, "clrMap"));
}

fn assert_mapped(model: &Presentation, text_slot: &str, fill_slot: &str) {
    assert_eq!(
        text(model).paragraphs[0].runs[0].style.color,
        rgb(text_slot)
    );
    let rectangles: Vec<_> = model.slide_contents()[0]
        .shapes()
        .iter()
        .filter_map(|s| match s {
            Shape::Rectangle { fill, stroke, .. }
                if matches!(fill, Some(Fill::Solid(_))) || stroke.is_some() =>
            {
                Some((fill, stroke))
            }
            _ => None,
        })
        .collect();
    assert_eq!(rectangles.len(), 2);
    for (fill, stroke) in rectangles {
        assert_eq!(*fill, Some(Fill::Solid(rgb(fill_slot))));
        assert_eq!(stroke.unwrap().color, rgb(text_slot));
        assert_eq!(stroke.unwrap().width, 2.0);
    }
    assert!(!warning(model, "clrMap"), "{:?}", model.warnings());
}

#[test]
fn mapped_text_fill_line_and_style_matrix_use_the_same_map() {
    for name in [
        "clrmap-text.pptx",
        "clrmap-fill.pptx",
        "clrmap-line.pptx",
        "clrmap-style-matrix.pptx",
    ] {
        assert_mapped(&parse(&fixture(name)).unwrap(), "accent2", "accent3");
    }
}

#[test]
fn layout_master_mapping_and_override_have_correct_precedence() {
    assert_mapped(
        &parse(&fixture("layout-master-clr-mapping.pptx")).unwrap(),
        "accent2",
        "accent3",
    );
    let model = parse(&fixture("layout-override-clr-mapping.pptx")).unwrap();
    assert_mapped(&model, "lt1", "accent4");
    assert_eq!(
        model.slide_layouts()[0].color_map().resolve_slot("tx1"),
        Some(ThemeColorSlot::Light1)
    );
}

#[test]
fn slide_master_mapping_resets_layout_override_and_slide_override_wins() {
    assert_mapped(
        &parse(&fixture("slide-master-clr-mapping.pptx")).unwrap(),
        "accent2",
        "accent3",
    );
    assert_mapped(
        &parse(&fixture("slide-override-clr-mapping.pptx")).unwrap(),
        "accent6",
        "accent5",
    );
}

#[test]
fn inherited_background_placeholder_text_and_decorations_use_slide_map() {
    let mut parts = fixture("master-body-style.pptx");
    append_root(
        &mut parts,
        "ppt/slides/slide1.xml",
        "sld",
        &format!(
            "<p:clrMapOvr><a:overrideClrMapping {}/></p:clrMapOvr>",
            color_mapping("accent6", "accent4")
        ),
    );
    let bg = "<p:bg><p:bgPr><a:solidFill><a:schemeClr val='tx1'/></a:solidFill></p:bgPr></p:bg>";
    rewrite(
        &mut parts,
        MASTER_PART,
        "<p:cSld>",
        &format!("<p:cSld>{bg}"),
    );
    let decoration = rectangle(
        5,
        NONTRIVIAL_RECT,
        &scheme("accent1", ""),
        "<a:ln><a:noFill/></a:ln>",
    );
    rewrite(
        &mut parts,
        MASTER_PART,
        "</p:spTree>",
        &format!("{decoration}</p:spTree>"),
    );
    let model = parse(&parts).unwrap();
    assert_eq!(
        model.slide_contents()[0].background(),
        Some(Fill::Solid(rgb("accent6")))
    );
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        rgb("accent6")
    );
    assert!(
        matches!(model.slide_contents()[0].shapes()[0], Shape::Rectangle { fill: Some(Fill::Solid(c)), .. } if c == rgb("accent4"))
    );
    assert_eq!(
        model.slide_masters()[0].background(),
        Some(Fill::Solid(rgb("accent2")))
    );
}

#[test]
fn layout_source_defaults_use_layout_map_and_slide_overrides_do_not_leak() {
    let mut parts = fixture("master-body-style.pptx");
    append_root(
        &mut parts,
        LAYOUT_PART,
        "sldLayout",
        &format!(
            "<p:clrMapOvr><a:overrideClrMapping {}/></p:clrMapOvr>",
            color_mapping("accent4", "accent1")
        ),
    );
    let second = parts
        .iter()
        .find(|(n, _)| n == "ppt/slides/slide1.xml")
        .unwrap()
        .1
        .clone();
    append_root(
        &mut parts,
        "ppt/slides/slide1.xml",
        "sld",
        &format!(
            "<p:clrMapOvr><a:overrideClrMapping {}/></p:clrMapOvr>",
            color_mapping("accent6", "accent1")
        ),
    );
    rewrite(
        &mut parts,
        "[Content_Types].xml",
        "</Types>",
        "<Override PartName='/ppt/slides/slide2.xml' ContentType='application/vnd.openxmlformats-officedocument.presentationml.slide+xml'/></Types>",
    );
    rewrite(
        &mut parts,
        "ppt/presentation.xml",
        "</p:sldIdLst>",
        "<p:sldId id='257' r:id='rId2'/></p:sldIdLst>",
    );
    let bytes = &mut parts
        .iter_mut()
        .find(|(n, _)| n == "ppt/_rels/presentation.xml.rels")
        .unwrap()
        .1;
    *bytes = String::from_utf8(bytes.clone())
        .unwrap()
        .replace(
            "</Relationships>",
            &format!(
                "{}</Relationships>",
                relation("rId2", "slide", "slides/slide2.xml")
            ),
        )
        .into_bytes();
    parts.push(("ppt/slides/slide2.xml".into(), second));
    parts.push((
        "ppt/slides/_rels/slide2.xml.rels".into(),
        rels(&relation(
            "rLayout",
            "slideLayout",
            "../slideLayouts/layout.xml",
        )),
    ));
    let model = parse(&parts).unwrap();
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        rgb("accent6")
    );
    let second_text = model.slide_contents()[1]
        .shapes()
        .iter()
        .find_map(|s| match s {
            Shape::TextBox(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        second_text.paragraphs[0].runs[0].style.color,
        rgb("accent4")
    );
    assert_eq!(
        model.slide_layouts()[0].placeholders()[0]
            .text_defaults()
            .color,
        Some(Some(rgb("accent4")))
    );
    assert_eq!(
        model.slide_masters()[0].placeholders()[0]
            .text_defaults()
            .color,
        Some(Some(rgb("accent2")))
    );
}

#[test]
fn malformed_mapping_warns_and_never_reveals_an_ancestor_mapping() {
    let mut parts = fixture("slide-override-clr-mapping.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "tx1='accent6'",
        "tx1='unknown'",
    );
    let model = parse(&parts).unwrap();
    assert!(warning(&model, "malformed color mapping"));
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        Color::rgb(0, 0, 0)
    );
    rewrite(&mut parts, "ppt/slides/slide1.xml", "tx1='unknown'", "");
    assert!(warning(&parse(&parts).unwrap(), "malformed color mapping"));
    let mut parts = map_deck();
    append_root(
        &mut parts,
        "ppt/slides/slide1.xml",
        "sld",
        "<p:clrMapOvr><a:masterClrMapping/><a:overrideClrMapping/></p:clrMapOvr>",
    );
    assert!(warning(&parse(&parts).unwrap(), "malformed clrMapOvr"));
}

#[test]
fn clrmap_transform_order_and_matrix_entry_scheme_colors_use_effective_mapping() {
    let mut parts = fixture("slide-override-clr-mapping.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:fillRef idx='1'>",
        "<a:fillRef idx='2'>",
    );
    // Map accent2 in the matrix entry itself; substitution is not the only path.
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "accent2='accent2'",
        "accent2='accent6'",
    );
    let model = parse(&parts).unwrap();
    let last = model.slide_contents()[0].shapes().last().unwrap();
    assert!(
        matches!(last, Shape::Rectangle { fill: Some(Fill::Solid(c)), .. } if *c == Color::rgb(16, 79, 87))
    );
}

#[test]
fn mapped_background_and_font_references_share_transform_and_lookup_rules() {
    let mut parts = map_deck();
    let shape = styled(
        &text_shape(
            2,
            TEXT_RECT,
            &inherited_paragraph("Font reference", "<a:pPr><a:defRPr sz='2000'/></a:pPr>"),
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        ),
        &style_ref("fontRef", "minor", "tx1"),
    );
    let shaded = rectangle(
        3,
        NONTRIVIAL_RECT,
        &scheme("tx1", "<a:shade val='50000'/>"),
        "<a:ln><a:noFill/></a:ln>",
    );
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        slide(
            &style_background("1002", "tx1"),
            &format!("{shape}{shaded}"),
        ),
    );
    let model = parse(&parts).unwrap();
    assert_eq!(
        model.slide_contents()[0].background(),
        Some(Fill::Solid(rgb("accent2")))
    );
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        rgb("accent2")
    );
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.family.as_deref(),
        Some("O2P Test Sans")
    );
    assert!(
        matches!(model.slide_contents()[0].shapes().last().unwrap(), Shape::Rectangle { fill: Some(Fill::Solid(c)), .. } if *c == Color::rgb(16, 72, 48))
    );
    assert!(!warning(&model, "text inheritance/font resolution"));
}

#[test]
fn semantic_tokens_map_once_to_a_theme_slot_without_recursive_remapping() {
    let mut parts = map_deck();
    rewrite(&mut parts, MASTER_PART, "tx1='accent2'", "tx1='accent1'");
    let model = parse(&parts).unwrap();
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        rgb("accent1")
    );
    assert!(model.slide_contents()[0].shapes().iter().any(
        |s| matches!(s, Shape::Rectangle { fill: Some(Fill::Solid(c)), .. } if *c == rgb("accent3"))
    ));
}

#[test]
fn strict_namespaces_use_the_same_text_and_mapping_semantics() {
    let mut parts = fixture("master-body-style.pptx");
    for (name, bytes) in &mut parts {
        if name.ends_with(".xml") {
            *bytes = String::from_utf8(bytes.clone())
                .unwrap()
                .replace(PML, "http://purl.oclc.org/ooxml/presentationml/main")
                .replace(DML, "http://purl.oclc.org/ooxml/drawingml/main")
                .replace(
                    OFFICE_RELS,
                    "http://purl.oclc.org/ooxml/officeDocument/relationships",
                )
                .into_bytes();
        }
    }
    let model = parse(&parts).unwrap();
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 32.0);
    assert_eq!(
        text(&model).paragraphs[0].runs[0].style.color,
        rgb("accent2")
    );
    assert!(!warning(&model, "text inheritance/font resolution"));
    assert!(!warning(&model, "clrMap"));
}

#[test]
fn missing_style_level_warns_instead_of_copying_level_one() {
    let mut parts = fixture("master-body-style.pptx");
    rewrite(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:p>",
        "<a:p><a:pPr lvl='8'/>",
    );
    let model = parse(&parts).unwrap();
    assert_eq!(text(&model).paragraphs[0].runs[0].style.font_size, 18.0);
    assert!(!text(&model).paragraphs[0].runs[0].style.bold);
    assert!(warning(&model, "text inheritance/font resolution"));
}

#[test]
fn fixtures_are_reproducible_and_unicode_rendering_preserves_searchable_text() {
    let mut fonts = o2p_fonts::FontBook::new();
    fonts
        .register_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"),
        )
        .unwrap();
    fonts.set_fallback_families(vec!["O2P Test Sans".into(), "O2P Test Japanese".into()]);
    for (name, parts) in fidelity2a_fixtures() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(
            std::fs::read(file).unwrap(),
            zip(&parts, true, false),
            "{name}"
        );
        let model = parse(&parts).unwrap();
        let report = model.to_display_list_with_fonts(&fonts).unwrap();
        let bytes = o2p_pdf::render(&report.display_list).unwrap();
        assert_eq!(bytes, o2p_pdf::render(&report.display_list).unwrap());
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 1);
        if name == "master-body-style.pptx" {
            assert!(pdf.extract_text(&[1]).unwrap().contains("Inherited 日本語"));
            assert!(bytes.windows(10).any(|s| s == b"/ToUnicode"));
        }
    }
}
