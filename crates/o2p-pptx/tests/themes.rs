mod support;

use o2p_core::{Color, DrawOp};
use o2p_fonts::{FontBook, FontWarning};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{
    Fill, ParseLimits, PptxError, PptxWarning, Presentation, Shape, TextLayoutWarningKind,
};
use std::path::PathBuf;
use support::*;

fn package(parts: &Parts) -> OoxmlPackage {
    OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default()).unwrap()
}
fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    Presentation::parse_with_content(&package(parts), ParseLimits::default())
}
fn parts(name: &str) -> Parts {
    theme_fixtures()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
fn hex(value: &str) -> Color {
    let n = u32::from_str_radix(value, 16).unwrap();
    Color::rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}
fn rewrite(parts: &mut Parts, name: &str, from: &str, to: &str) {
    let bytes = &mut parts.iter_mut().find(|(n, _)| n == name).unwrap().1;
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains(from), "{name}: {from}");
    *bytes = text.replace(from, to).into_bytes();
}
fn fonts() -> FontBook {
    let mut fonts = FontBook::new();
    fonts
        .register_directory(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
        .unwrap();
    fonts.set_fallback_families(vec!["O2P Test Sans".into(), "O2P Test Japanese".into()]);
    fonts
}
fn first_fill(model: &Presentation) -> Option<Fill> {
    match model.slide_contents()[0].shapes()[0] {
        Shape::Rectangle { fill, .. } => fill,
        _ => panic!("expected rectangle"),
    }
}
fn paint(paint: &str, theme: &str) -> Result<Presentation, PptxError> {
    parse(&attach_theme(
        content_deck(
            "",
            &rectangle(2, NONTRIVIAL_RECT, paint, "<a:ln><a:noFill/></a:ln>"),
        ),
        theme,
    ))
}

#[test]
fn fixtures_are_reproducible_and_owned_theme_survives_package() {
    for (name, parts) in theme_fixtures() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(std::fs::read(path).unwrap(), zip(&parts, true, false));
        let result = parse(&parts);
        if name == "malformed-theme.pptx" {
            assert!(matches!(result, Err(PptxError::InvalidTheme { .. })));
        } else {
            let model = result.unwrap();
            assert!(o2p_pdf::render_with_report(&model.to_display_list()).is_ok());
            if name != "missing-theme.pptx" {
                assert_eq!(model.clone().theme(), model.theme());
            }
        }
    }
}

#[test]
fn presentation_relationship_discovers_relocated_theme_and_main_parts() {
    let mut p = parts("theme-accent-fill.pptx");
    rewrite(
        &mut p,
        "[Content_Types].xml",
        "/ppt/presentation.xml",
        "/decks/pres.xml",
    );
    rewrite(
        &mut p,
        "[Content_Types].xml",
        THEME_PART,
        "assets/palette.xml",
    );
    rewrite(
        &mut p,
        "_rels/.rels",
        "ppt/presentation.xml",
        "decks/pres.xml",
    );
    rewrite(
        &mut p,
        "ppt/_rels/presentation.xml.rels",
        "slides/slide1.xml",
        "../ppt/slides/slide1.xml",
    );
    rewrite(
        &mut p,
        "ppt/_rels/presentation.xml.rels",
        "theme/synthetic.xml",
        "../assets/palette.xml",
    );
    for (name, _) in &mut p {
        *name = match name.as_str() {
            "ppt/presentation.xml" => "decks/pres.xml".into(),
            "ppt/_rels/presentation.xml.rels" => "decks/_rels/pres.xml.rels".into(),
            THEME_PART => "assets/palette.xml".into(),
            _ => name.clone(),
        };
    }
    let model = parse(&p).unwrap();
    assert_eq!(first_fill(&model), Some(Fill::Solid(hex("2060D0"))));
    assert_eq!(model.theme().unwrap().colors.colors.len(), 12);
}

#[test]
fn all_standard_slots_parse_srgb_system_lastclr_and_resolve_in_slide_paint() {
    let model = parse(&parts("multiple-theme-colors.pptx")).unwrap();
    let theme = model.theme().unwrap();
    assert_eq!(theme.colors.colors.len(), 12);
    for ((slot, value), shape) in THEME_COLORS
        .into_iter()
        .zip(model.slide_contents()[0].shapes())
    {
        assert_eq!(theme.resolve_color(slot), Some(hex(value)), "{slot}");
        assert!(
            matches!(shape, Shape::Rectangle { fill: Some(Fill::Solid(c)), .. } if *c == hex(value))
        );
    }
    assert_eq!(theme.resolve_color("bg1"), None);
    assert_eq!(theme.resolve_color("tx1"), None);
    assert_eq!(theme.resolve_color("phClr"), None);
    assert_eq!(theme.resolve_color("invented"), None);
}

#[test]
fn direct_srgb_has_priority_and_remains_independent_of_theme() {
    let themed = paint(&solid("a1B2c3"), &theme_xml()).unwrap();
    let plain = parse(&content_deck(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("a1B2c3"),
            "<a:ln><a:noFill/></a:ln>",
        ),
    ))
    .unwrap();
    assert_eq!(first_fill(&themed), Some(Fill::Solid(hex("A1B2C3"))));
    assert_eq!(themed.to_display_list(), plain.to_display_list());
    assert!(
        paint(
            "<a:solidFill><a:srgbClr val='112233'/><a:schemeClr val='accent1'/></a:solidFill>",
            &theme_xml()
        )
        .is_err()
    );
}

#[test]
fn backgrounds_fills_ellipse_outlines_lines_and_text_share_resolution() {
    let model = parse(&parts("theme-colors-and-fonts.pptx")).unwrap();
    assert_eq!(
        model.slide_contents()[0].background(),
        Some(Fill::Solid(hex("F7F9FC")))
    );
    let display = model.to_display_list();
    assert!(matches!(display.pages[0].ops[0], DrawOp::Rect(ref r) if r.color == hex("F7F9FC")));
    assert!(
        display.pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Line(line) if line.color == hex("209060")))
    );
    let model = parse(&parts("theme-outline.pptx")).unwrap();
    let display = model.to_display_list();
    assert!(
        matches!(display.pages[0].ops[0], DrawOp::StrokeEllipse(ref s) if s.color == hex("209060"))
    );
    assert!(matches!(display.pages[0].ops[1], DrawOp::Line(ref s) if s.color == hex("209060")));
    let rect = paint(&scheme("accent1", ""), &theme_xml()).unwrap();
    assert_eq!(first_fill(&rect), Some(Fill::Solid(hex("2060D0"))));
    let ellipse = parse(&attach_theme(
        content_deck(
            "",
            &ellipse(
                2,
                NONTRIVIAL_RECT,
                &scheme("accent3", ""),
                &format!("<a:ln w='12700'>{}</a:ln>", scheme("accent5", "")),
            ),
        ),
        &theme_xml(),
    ))
    .unwrap();
    assert!(
        matches!(ellipse.to_display_list().pages[0].ops[0], DrawOp::Ellipse(ref e) if e.color == hex("E88A20"))
    );
    let text = parse(&parts("theme-text-color.pptx")).unwrap();
    assert!(
        text.to_display_list().pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Text(t) if t.color == hex("C43D56")))
    );
    let report = text.to_display_list_with_fonts(&fonts()).unwrap();
    assert!(
        report.display_list.pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::GlyphText(t) if t.color == hex("C43D56")))
    );
}

#[test]
fn color_transforms_resolve_in_order_on_direct_and_theme_definitions() {
    for (transform, value) in [
        ("<a:tint val='50000'/>", "FF8080"),
        ("<a:shade val='50000'/>", "800000"),
        ("<a:lumMod val='50000'/>", "800000"),
        ("<a:lumOff val='25000'/>", "FF8080"),
        ("<a:lumMod val='50000'/><a:lumOff val='50000'/>", "FF8080"),
        ("<a:lumOff val='50000'/><a:lumMod val='50000'/>", "FF0000"),
    ] {
        let direct = paint(
            &format!("<a:solidFill><a:srgbClr val='FF0000'>{transform}</a:srgbClr></a:solidFill>"),
            &theme_xml(),
        )
        .unwrap();
        let theme = theme_xml().replace(
            "<a:srgbClr val='2060D0'/>",
            &format!("<a:srgbClr val='FF0000'>{transform}</a:srgbClr>"),
        );
        let themed = paint(&scheme("accent1", ""), &theme).unwrap();
        assert_eq!(first_fill(&direct), Some(Fill::Solid(hex(value))));
        assert_eq!(first_fill(&direct), first_fill(&themed));
    }
    let theme = theme_xml().replace("2060D0", "FF0000");
    assert_eq!(
        first_fill(&paint(&scheme("accent1", "<a:shade val='50000'/>"), &theme).unwrap()),
        Some(Fill::Solid(hex("800000")))
    );
}

#[test]
fn major_minor_latin_and_script_mappings_parse_and_tokens_become_generic_families() {
    for (fixture, token) in [
        ("theme-major-font.pptx", "+mj-lt"),
        ("theme-minor-font.pptx", "+mn-lt"),
    ] {
        let model = parse(&parts(fixture)).unwrap();
        let theme = model.theme().unwrap();
        assert_eq!(theme.fonts.major.latin.as_deref(), Some("O2P Test Sans"));
        assert_eq!(theme.fonts.minor.latin.as_deref(), Some("O2P Test Sans"));
        assert_eq!(theme.fonts.major.scripts["Jpan"], "O2P Test Japanese");
        assert_eq!(theme.fonts.minor.scripts["Jpan"], "O2P Test Japanese");
        assert_eq!(theme.resolve_font(token), Some("O2P Test Sans"));
        let Shape::TextBox(text) = &model.slide_contents()[0].shapes()[1] else {
            panic!("text");
        };
        assert_eq!(
            text.paragraphs[0].runs[0].style.family.as_deref(),
            Some("O2P Test Sans")
        );
        let report = model.to_display_list_with_fonts(&fonts()).unwrap();
        assert!(
            !report
                .warnings
                .iter()
                .any(|w| matches!(w.kind, TextLayoutWarningKind::Font(_)))
        );
        let bytes = o2p_pdf::render(&report.display_list).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(pdf.extract_text(&[1]).unwrap().contains("theme font"));
        assert!(bytes.windows(10).any(|w| w == b"/FontFile2"));
        assert!(bytes.windows(10).any(|w| w == b"/ToUnicode"));
    }
    let theme = theme_xml().replace(
        "<a:minorFont><a:latin typeface='O2P Test Sans'",
        "<a:minorFont><a:latin typeface='Minor Separate Family'",
    );
    let model = parse(&attach_theme(
        content_deck("", &theme_text(2, TEXT_RECT, "+mn-lt", "dk1", "Minor")),
        &theme,
    ))
    .unwrap();
    assert_eq!(
        model.theme().unwrap().resolve_font("+mj-lt"),
        Some("O2P Test Sans")
    );
    assert_eq!(
        model.theme().unwrap().resolve_font("+mn-lt"),
        Some("Minor Separate Family")
    );
}

#[test]
fn unavailable_real_theme_family_retains_existing_substitution_diagnostics() {
    for token in ["+mj-lt", "+mn-lt"] {
        let theme = theme_xml().replace("O2P Test Sans", "Unregistered Theme Family");
        let model = parse(&attach_theme(
            content_deck("", &theme_text(2, TEXT_RECT, token, "dk1", "Résumé ffi")),
            &theme,
        ))
        .unwrap();
        let report = model.to_display_list_with_fonts(&fonts()).unwrap();
        assert!(report.warnings.iter().any(|w| matches!(&w.kind, TextLayoutWarningKind::Font(FontWarning::MissingFont { family }) if family == "Unregistered Theme Family")));
        assert!(report.warnings.iter().any(|w| matches!(&w.kind, TextLayoutWarningKind::Font(FontWarning::Substitution { requested, selected_family, .. }) if requested.family == "Unregistered Theme Family" && selected_family == "O2P Test Sans")));
        assert_eq!(report, model.to_display_list_with_fonts(&fonts()).unwrap());
    }
}

#[test]
fn missing_theme_missing_slot_and_unsupported_aliases_warn_with_documented_fallbacks() {
    let model = parse(&parts("missing-theme.pptx")).unwrap();
    assert!(model.theme().is_none());
    assert_eq!(first_fill(&model), None);
    let Shape::TextBox(text) = &model.slide_contents()[0].shapes()[2] else {
        panic!("text");
    };
    assert_eq!(text.paragraphs[0].runs[0].style.color, Color::BLACK);
    assert_eq!(text.paragraphs[0].runs[0].style.family, None);
    assert!(model.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedSlideContent { slide_id: 256, part, name } if part == "ppt/slides/slide1.xml" && name.contains("schemeClr"))));
    assert!(
        model
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("+mj-lt unresolved"))
    );
    for slot in ["bg1", "tx1", "phClr", "not-a-slot"] {
        let model = paint(&scheme(slot, ""), &theme_xml()).unwrap();
        assert_eq!(first_fill(&model), None);
        assert!(
            model
                .warnings()
                .iter()
                .any(|w| w.to_string().contains("theme color unavailable"))
        );
    }
    for token in ["+mj-ea", "+mn-cs", "+unknown"] {
        let model = parse(&attach_theme(
            content_deck("", &theme_text(2, TEXT_RECT, token, "dk1", "A")),
            &theme_xml(),
        ))
        .unwrap();
        let Shape::TextBox(text) = &model.slide_contents()[0].shapes()[1] else {
            panic!("text");
        };
        assert_eq!(text.paragraphs[0].runs[0].style.family, None);
        assert!(
            model
                .warnings()
                .iter()
                .any(|w| w.to_string().contains(token))
        );
    }
    let missing = theme_xml().replace("<a:accent1><a:srgbClr val='2060D0'/></a:accent1>", "");
    let model = paint(&scheme("accent1", ""), &missing).unwrap();
    assert_eq!(first_fill(&model), None);
    assert!(model.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedTheme { name, .. } if name.contains("missing color-scheme slot accent1"))));
}

#[test]
fn unsupported_theme_definitions_and_transforms_are_structured_warnings() {
    for definition in [
        "<a:hslClr hue='0' sat='100000' lum='50000'/>",
        "<a:scrgbClr r='100000' g='0' b='0'/>",
        "<a:prstClr val='red'/>",
        "<a:schemeClr val='accent2'/>",
        "<a:sysClr val='window'/>",
        "<a:srgbClr val='2060D0'><a:alpha val='50000'/></a:srgbClr>",
    ] {
        let theme = theme_xml().replace("<a:srgbClr val='2060D0'/>", definition);
        let model = paint(&scheme("accent1", ""), &theme).unwrap();
        assert_eq!(first_fill(&model), None);
        assert!(model.warnings().iter().any(
            |w| matches!(w, PptxWarning::UnsupportedTheme { part, .. } if part == THEME_PART)
        ));
    }
    for transform in ["alpha", "satMod", "gamma", "hueOff", "unknown"] {
        let model = paint(
            &scheme("accent1", &format!("<a:{transform} val='50000'/>")),
            &theme_xml(),
        )
        .unwrap();
        assert_eq!(first_fill(&model), None);
        assert!(model.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedSlideContent { name, .. } if name.contains(transform))));
    }
    let model = parse(&parts("theme-accent-fill.pptx")).unwrap();
    assert!(!model.warnings().iter().any(
        |w| matches!(w, PptxWarning::UnsupportedTheme { name, .. } if name.contains("fmtScheme"))
    ));
}

#[test]
fn malformed_supported_theme_values_and_structures_fail_without_panics() {
    let original = theme_xml();
    for (from, to) in [
        ("val='2060D0'", "val='XYZ'"),
        ("<a:accent1>", "<a:accent1><a:srgbClr val='123456'/>"),
        ("<a:fontScheme", "<a:missingScheme"),
        ("</a:fontScheme>", "</a:missingScheme>"),
        ("<a:theme ", "<a:notTheme "),
        (
            "<a:majorFont>",
            "<a:majorFont><a:latin typeface='Duplicate'/>",
        ),
        ("script='Jpan'", "script=''"),
        ("typeface='O2P Test Sans'", "typeface='+mj-lt'"),
    ] {
        assert!(matches!(
            paint(&scheme("accent1", ""), &original.replace(from, to)),
            Err(PptxError::InvalidTheme { .. } | PptxError::InvalidXml { .. })
        ));
    }
    for transform in [
        "<a:tint val='-1'/>",
        "<a:shade val='100001'/>",
        "<a:lumMod val='-1'/>",
        "<a:lumOff val='2147483648'/>",
        "<a:tint val='bad'/>",
        "<a:shade/>",
        "<a:lumOff val='0'><a:child/></a:lumOff>",
    ] {
        assert!(matches!(
            paint(&scheme("accent1", transform), &original),
            Err(PptxError::InvalidSlide { .. })
        ));
    }
    let mut duplicate = parts("theme-accent-fill.pptx");
    rewrite(
        &mut duplicate,
        "ppt/_rels/presentation.xml.rels",
        "</Relationships>",
        &format!(
            "{}</Relationships>",
            relation("rTheme2", "theme", "theme/synthetic.xml")
        ),
    );
    assert!(matches!(
        parse(&duplicate),
        Err(PptxError::InvalidTheme { .. })
    ));
    let mut wrong_type = parts("theme-accent-fill.pptx");
    rewrite(
        &mut wrong_type,
        "[Content_Types].xml",
        "application/vnd.openxmlformats-officedocument.theme+xml",
        "application/xml",
    );
    assert!(matches!(
        parse(&wrong_type),
        Err(PptxError::InvalidTheme { .. })
    ));
}

#[test]
fn theme_parts_use_existing_xml_limits_namespaces_and_external_target_policy() {
    let mut strict = parts("theme-accent-fill.pptx");
    rewrite(
        &mut strict,
        THEME_PART,
        DML,
        "http://purl.oclc.org/ooxml/drawingml/main",
    );
    rewrite(
        &mut strict,
        "ppt/_rels/presentation.xml.rels",
        &format!("{OFFICE_RELS}/theme"),
        "http://purl.oclc.org/ooxml/officeDocument/relationships/theme",
    );
    assert_eq!(
        first_fill(&parse(&strict).unwrap()),
        Some(Fill::Solid(hex("2060D0")))
    );
    let mut utf16 = parts("theme-accent-fill.pptx");
    let text = theme_xml();
    replace(
        &mut utf16,
        THEME_PART,
        [
            vec![0xff, 0xfe],
            text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat(),
    );
    assert!(parse(&utf16).is_ok());
    let mut external = parts("theme-accent-fill.pptx");
    rewrite(
        &mut external,
        "ppt/_rels/presentation.xml.rels",
        "Target='theme/synthetic.xml'",
        "Target='https://example.invalid/theme' TargetMode='External'",
    );
    let model = parse(&external).unwrap();
    assert!(model.theme().is_none());
    assert_eq!(first_fill(&model), None);
    assert!(
        model
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("never fetched"))
    );
    let mut hostile = parts("theme-accent-fill.pptx");
    replace(
        &mut hostile,
        THEME_PART,
        format!("<!DOCTYPE a:theme []>{}", theme_xml()).into_bytes(),
    );
    assert!(matches!(parse(&hostile), Err(PptxError::InvalidXml { .. })));
    replace(
        &mut hostile,
        THEME_PART,
        format!("{}{}", "<n>".repeat(65), "</n>".repeat(65)).into_bytes(),
    );
    assert!(matches!(
        parse(&hostile),
        Err(PptxError::LimitExceeded {
            limit: "max_xml_depth"
        })
    ));
    let p = package(&parts("theme-accent-fill.pptx"));
    let limits = ParseLimits {
        max_xml_nodes: 40,
        ..ParseLimits::default()
    };
    assert!(matches!(
        Presentation::parse_with_content(&p, limits),
        Err(PptxError::LimitExceeded {
            limit: "max_xml_nodes"
        })
    ));
    let limits = ParseLimits {
        max_xml_size: theme_xml().len() - 1,
        ..ParseLimits::default()
    };
    assert!(matches!(
        Presentation::parse_with_content(&p, limits),
        Err(PptxError::LimitExceeded {
            limit: "max_xml_size"
        })
    ));
}

#[test]
fn metadata_inspection_remains_opaque_and_theme_does_not_synthesize_inheritance() {
    let p = package(&parts("malformed-theme.pptx"));
    assert!(p.inspect_pptx().is_ok());
    let metadata = Presentation::parse(&p, ParseLimits::default()).unwrap();
    assert!(metadata.theme().is_none());
    assert!(metadata.to_blank_display_list().pages[0].ops.is_empty());
    let model = parse(&attach_theme(
        content_deck(
            "",
            &text_shape(
                2,
                TEXT_RECT,
                "<a:p><a:r><a:t>No inherited theme font</a:t></a:r></a:p>",
                "<a:noFill/>",
                "<a:ln><a:noFill/></a:ln>",
            ),
        ),
        &theme_xml(),
    ))
    .unwrap();
    let Shape::TextBox(text) = &model.slide_contents()[0].shapes()[1] else {
        panic!("text");
    };
    assert_eq!(text.paragraphs[0].runs[0].style.family, None);
    assert_eq!(text.paragraphs[0].runs[0].style.color, Color::BLACK);
}

#[test]
fn themed_pdf_is_deterministic_with_color_operators_embedded_font_and_unicode_mapping() {
    let model = parse(&parts("theme-colors-and-fonts.pptx")).unwrap();
    let report = model.to_display_list_with_fonts(&fonts()).unwrap();
    let bytes = o2p_pdf::render(&report.display_list).unwrap();
    assert_eq!(bytes, o2p_pdf::render(&report.display_list).unwrap());
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let page = pdf.get_pages()[&1];
    let content = lopdf::content::Content::decode(&pdf.get_page_content(page).unwrap()).unwrap();
    for (_, hex_value) in THEME_COLORS {
        let c = hex(hex_value);
        let expected = [c.red, c.green, c.blue].map(|v| f32::from(v) / 255.0);
        assert!(content.operations.iter().any(|op| op.operator == "rg"
            && op.operands.iter().zip(expected).all(|(actual, expected)| {
                (actual.as_float().unwrap() - expected).abs() < 0.00001
            })));
    }
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert!(
        extracted.contains("Résumé")
            && extracted.contains("Major Latin")
            && extracted.contains("Minor Latin")
    );
    assert!(bytes.windows(10).any(|w| w == b"/FontFile2"));
    assert!(bytes.windows(10).any(|w| w == b"/ToUnicode"));
    assert!(bytes.windows(11).any(|w| w == b"/ActualText"));
}
