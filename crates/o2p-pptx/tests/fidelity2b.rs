mod support;
use o2p_core::{Color, DrawOp, ShadowGeometry};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{ParseLimits, Presentation, Shape};
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, o2p_pptx::PptxError> {
    let package =
        OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default())
}
fn model(name: &str) -> Presentation {
    let (_, p) = fidelity2b_fixtures()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap();
    parse(&p).unwrap()
}
fn shadow(model: &Presentation) -> o2p_core::ShadowOp {
    model.to_display_list().pages[0]
        .ops
        .iter()
        .find_map(|o| {
            if let DrawOp::Shadow(s) = o {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap()
}
#[test]
fn round_rect_default_adjustment_fill_and_outline() {
    for (name, adjustment) in [
        ("rounded-rectangle.pptx", 0.16667),
        ("rounded-rectangle-custom-radius.pptx", 0.12),
    ] {
        let m = model(name);
        let Shape::RoundedRectangle {
            rect,
            radius,
            fill,
            stroke,
        } = &m.slide_contents()[0].shapes()[0]
        else {
            panic!("rounded shape")
        };
        assert!((radius - rect.size.width.min(rect.size.height) * adjustment).abs() < 0.0001);
        assert_eq!(*fill, Some(o2p_pptx::Fill::Solid(Color::rgb(32, 96, 208))));
        assert_eq!(stroke.unwrap().width, 2.0);
    }
}
#[test]
fn rounded_geometry_and_text_share_transform_and_stay_vector() {
    let m = model("rounded-rectangle-with-text.pptx");
    let d = m.to_display_list();
    assert!(matches!(d.pages[0].ops[0], DrawOp::RoundedRect(_)));
    assert!(d.pages[0].ops.iter().any(|o| matches!(o, DrawOp::Text(_))));
    let pdf = lopdf::Document::load_mem(&o2p_pdf::render(&d).unwrap()).unwrap();
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap();
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|o| o.operator == "c")
            .count(),
        8
    ); // fill + outline
    assert!(!content.operations.iter().any(|o| o.operator == "Do"));
    let m = model("rotated-rounded-rectangle.pptx");
    let d = m.to_display_list();
    assert!(matches!(
        d.pages[0].ops.first(),
        Some(DrawOp::BeginTransform(_))
    ));
    assert!(matches!(d.pages[0].ops.last(), Some(DrawOp::EndTransform)));
    let (_, mut p) = fidelity2b_fixtures()
        .into_iter()
        .find(|(n, _)| *n == "rounded-rectangle-with-text.pptx")
        .unwrap();
    let s = String::from_utf8(
        p.iter()
            .find(|(n, _)| n == "ppt/slides/slide1.xml")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap()
    .replace("<a:xfrm>", "<a:xfrm rot='5400000' flipV='1'>");
    replace(&mut p, "ppt/slides/slide1.xml", s.into_bytes());
    let d = parse(&p).unwrap().to_display_list();
    assert!(matches!(d.pages[0].ops[0], DrawOp::BeginTransform(_)));
    assert!(matches!(d.pages[0].ops.last(), Some(DrawOp::EndTransform)));
    assert!(d.pages[0].ops.iter().any(|o| matches!(o, DrawOp::Text(_))));
}
#[test]
fn outer_shadow_units_alpha_geometry_and_order() {
    for name in [
        "rectangle-outer-shadow.pptx",
        "rounded-rectangle-shadow.pptx",
        "ellipse-shadow.pptx",
        "image-shadow.pptx",
    ] {
        let m = model(name);
        let d = m.to_display_list();
        let s = shadow(&m);
        assert_eq!(s.shadow.color, Color::rgb(16, 32, 48));
        assert_eq!(s.shadow.opacity, 0.35);
        assert_eq!(s.shadow.blur_radius, 40000.0 / 12700.0);
        assert!(s.shadow.offset.x.abs() < 0.00001);
        assert_eq!(s.shadow.offset.y, 2.0);
        assert!(matches!(
            d.pages[0]
                .ops
                .iter()
                .find(|o| !matches!(o, DrawOp::Rect(_))),
            Some(DrawOp::Shadow(_))
        ));
        match name {
            "image-shadow.pptx" => assert!(matches!(s.geometry, ShadowGeometry::Image(_))),
            "ellipse-shadow.pptx" => assert!(matches!(s.geometry, ShadowGeometry::Ellipse(_))),
            _ => {}
        }
        o2p_pdf::render(&d).unwrap();
    }
    let d = model("overlapping-shadow-zorder.pptx").to_display_list();
    assert!(matches!(
        d.pages[0].ops.as_slice(),
        [
            DrawOp::Shadow(_),
            DrawOp::RoundedRect(_),
            DrawOp::StrokeRoundedRect(_),
            DrawOp::Rect(_)
        ]
    ));
}
#[test]
fn theme_reference_and_local_precedence() {
    assert_eq!(
        shadow(&model("theme-shadow-style.pptx")).shadow.color,
        Color::rgb(32, 144, 96)
    );
    assert_eq!(
        shadow(&model("local-shadow-overrides-theme.pptx"))
            .shadow
            .color,
        Color::rgb(255, 0, 0)
    );
    let rect = rounded(
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("2060D0"),
            "<a:ln><a:noFill/></a:ln>",
        ),
        "",
    );
    let themed = styled(&rect, &style_ref("effectRef", "1", "accent2"));
    let m = parse(&shadow_style_deck(&effect_shape(&themed, "<a:effectLst/>"))).unwrap();
    assert!(
        !m.to_display_list().pages[0]
            .ops
            .iter()
            .any(|o| matches!(o, DrawOp::Shadow(_)))
    );
}
#[test]
fn custom_formulas_warn_no_fill_and_clamped_radius_work() {
    for (guide, expected, warning) in [
        ("<a:gd name='adj' fmla='val 99999'/>", 0.5, false),
        ("<a:gd name='adj' fmla='val -1'/>", 0.0, false),
        ("<a:gd name='adj' fmla='*/ ss 1 2'/>", 0.16667, true),
    ] {
        let m = parse(&content_deck(
            "",
            &rounded(
                &rectangle(2, NONTRIVIAL_RECT, "<a:noFill/>", &outline("000000", 12700)),
                guide,
            ),
        ))
        .unwrap();
        let d = m.to_display_list();
        let DrawOp::StrokeRoundedRect(r) = d.pages[0].ops[0] else {
            panic!()
        };
        assert!((r.radius - r.rect.size.width.min(r.rect.size.height) * expected).abs() < 0.0001);
        assert_eq!(
            m.warnings()
                .iter()
                .any(|w| w.to_string().contains("custom adjustment")),
            warning
        );
    }
}
#[test]
fn malformed_extreme_shadows_and_duplicate_adjustments() {
    let r = rectangle(
        2,
        NONTRIVIAL_RECT,
        &solid("2060D0"),
        "<a:ln><a:noFill/></a:ln>",
    );
    for invalid in [
        SHADOW_LIST.replace("40000", "-1"),
        SHADOW_LIST.replace("40000", "99999999999999"),
        SHADOW_LIST.replace("5400000", "21600000"),
        SHADOW_LIST.replace("35000", "100001"),
        SHADOW_LIST.replace("rotWithShape='0'", "rotWithShape='bad'"),
        "<a:effectLst><a:outerShdw/></a:effectLst>".into(),
    ] {
        assert!(parse(&content_deck("", &effect_shape(&r, &invalid))).is_err());
    }
    let m = parse(&content_deck(
        "",
        &effect_shape(
            &r,
            &SHADOW_LIST
                .replace("blurRad='40000'", "blurRad='4294967295'")
                .replace("25400", "4294967295"),
        ),
    ))
    .unwrap();
    o2p_pdf::render(&m.to_display_list()).unwrap();
    let g = "<a:gd name='adj' fmla='val 1'/><a:gd name='adj' fmla='val 2'/>";
    assert!(parse(&content_deck("", &rounded(&r, g))).is_err());
    let m = parse(&content_deck(
        "",
        &effect_shape(
            &r,
            &SHADOW_LIST.replace("dir='5400000'", "dir='5400000' sx='110000'"),
        ),
    ))
    .unwrap();
    assert!(
        !m.to_display_list().pages[0]
            .ops
            .iter()
            .any(|o| matches!(o, DrawOp::Shadow(_)))
    );
    assert!(
        m.warnings()
            .iter()
            .any(|w| w.to_string().contains("scaled/skewed"))
    );
}
#[test]
fn shadow_offset_is_page_relative_when_shape_rotates_and_flips() {
    let r = rounded(
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("2060D0"),
            "<a:ln><a:noFill/></a:ln>",
        ),
        "",
    )
    .replace("<a:xfrm>", "<a:xfrm rot='5400000' flipH='1'>");
    let m = parse(&content_deck("", &effect_shape(&r, SHADOW_LIST))).unwrap();
    let d = m.to_display_list();
    let DrawOp::BeginTransform(t) = d.pages[0].ops[0] else {
        panic!()
    };
    let s = shadow(&m).shadow.offset;
    assert!((t.a * s.x + t.c * s.y).abs() < 0.0001);
    assert!((t.b * s.x + t.d * s.y - 2.0).abs() < 0.0001);
}
#[test]
fn fixtures_reproduce_and_render_deterministically() {
    for (name, parts) in fidelity2b_fixtures() {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/pptx")
                .join(name),
        )
        .unwrap();
        assert_eq!(bytes, zip(&parts, true, false));
        let m = parse(&parts).unwrap();
        let d = m.to_display_list();
        assert_eq!(o2p_pdf::render(&d).unwrap(), o2p_pdf::render(&d).unwrap());
    }
}

#[test]
fn strict_namespaces_and_direct_scheme_colors_share_theme_resolution() {
    let r = rectangle(
        2,
        NONTRIVIAL_RECT,
        &solid("2060D0"),
        "<a:ln><a:noFill/></a:ln>",
    );
    let effect = "<a:effectLst><a:outerShdw><a:schemeClr val='accent2'><a:alpha val='25000'/></a:schemeClr></a:outerShdw></a:effectLst>";
    let mut p = shadow_style_deck(&effect_shape(&r, effect));
    for (_, bytes) in p.iter_mut().filter(|(name, _)| name.ends_with(".xml")) {
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
    let m = parse(&p).unwrap();
    let s = shadow(&m);
    assert_eq!(s.shadow.color, Color::rgb(32, 144, 96));
    assert_eq!(s.shadow.opacity, 0.25);
    assert_eq!(s.shadow.offset, o2p_core::Point { x: 0.0, y: 0.0 });
    assert_eq!(s.shadow.blur_radius, 0.0);
    assert_eq!(m.theme().unwrap().styles.effect_style_count(), 1);
}

#[test]
fn placeholder_inherits_rounded_geometry_and_local_empty_effect_suppresses_parent() {
    let local = placeholder(2, "body", Some(1), "", None);
    let properties = "<a:xfrm><a:off x='914400' y='914400'/><a:ext cx='2743200' cy='1828800'/></a:xfrm><a:prstGeom prst='roundRect'><a:avLst><a:gd name='adj' fmla='val 12000'/></a:avLst></a:prstGeom><a:solidFill><a:srgbClr val='2060D0'/></a:solidFill><a:ln><a:noFill/></a:ln>";
    let parent = placeholder(
        2,
        "body",
        Some(1),
        &format!("{properties}{SHADOW_LIST}"),
        None,
    );
    let p = inheritance_deck("", &local, "", &parent, "", "");
    let m = parse(&p).unwrap();
    let d = m.to_display_list();
    assert!(matches!(
        d.pages[0].ops.as_slice(),
        [DrawOp::Shadow(_), DrawOp::RoundedRect(_)]
    ));
    let DrawOp::RoundedRect(r) = d.pages[0].ops[1] else {
        panic!()
    };
    assert!((r.radius - r.rect.size.height * 0.12).abs() < 0.0001);
    let local = local.replace("<p:spPr></p:spPr>", "<p:spPr><a:effectLst/></p:spPr>");
    let m = parse(&inheritance_deck("", &local, "", &parent, "", "")).unwrap();
    assert!(
        !m.to_display_list().pages[0]
            .ops
            .iter()
            .any(|o| matches!(o, DrawOp::Shadow(_)))
    );
}

#[test]
fn out_of_scope_line_and_text_shadows_are_diagnosed() {
    let line = straight_line(2, NONTRIVIAL_RECT, &outline("000000", 12700));
    let text = text_shape(
        2,
        TEXT_RECT,
        &format!("<a:p>{}</a:p>", text_run("Text", "sz='1800'")),
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    );
    for (shape, message) in [
        (line, "line geometry unsupported"),
        (text, "text shadow unsupported"),
    ] {
        let m = parse(&content_deck("", &effect_shape(&shape, SHADOW_LIST))).unwrap();
        assert!(m.warnings().iter().any(|w| w.to_string().contains(message)));
        assert!(
            !m.to_display_list().pages[0]
                .ops
                .iter()
                .any(|o| matches!(o, DrawOp::Shadow(_)))
        );
    }
}
