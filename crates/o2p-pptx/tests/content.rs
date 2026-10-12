mod support;

use lopdf::{Document, Object, content::Content};
use o2p_core::{Color, DrawOp, Point, Rect, Size};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{Fill, ParseLimits, PptxError, PptxWarning, Presentation, Shape, emu_to_points};
use std::path::Path;
use support::{NONTRIVIAL_RECT, content_deck, rectangle, replace, slide, solid, zip};

fn parse(parts: &support::Parts, limits: ParseLimits) -> Result<Presentation, PptxError> {
    let package = OoxmlPackage::from_bytes(&zip(parts, false, false), SecurityLimits::default())?;
    Presentation::parse_with_content(&package, limits)
}

fn model(background: &str, shapes: &str) -> Presentation {
    parse(&content_deck(background, shapes), ParseLimits::default()).unwrap()
}

fn fixture(name: &str) -> Presentation {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/pptx")
        .join(name);
    let package = OoxmlPackage::open(path, SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default()).unwrap()
}

fn expected_rect() -> Rect {
    let (x, y, w, h) = NONTRIVIAL_RECT;
    // Independent reference arithmetic, without the production unit helper.
    Rect {
        origin: Point {
            x: (x as f64 / 12700.0) as f32,
            y: (y as f64 / 12700.0) as f32,
        },
        size: Size {
            width: (w as f64 / 12700.0) as f32,
            height: (h as f64 / 12700.0) as f32,
        },
    }
}

fn pdf_content(model: &Presentation) -> Vec<Content<Vec<lopdf::content::Operation>>> {
    let pdf = Document::load_mem(&o2p_pdf::render(&model.to_display_list()).unwrap()).unwrap();
    pdf.get_pages()
        .values()
        .map(|id| Content::decode(&pdf.get_page_content(*id).unwrap()).unwrap())
        .collect()
}

fn operands(content: &Content<Vec<lopdf::content::Operation>>, operator: &str) -> Vec<Vec<f32>> {
    content
        .operations
        .iter()
        .filter(|op| op.operator == operator)
        .map(|op| numbers(&op.operands))
        .collect()
}

fn numbers(objects: &[Object]) -> Vec<f32> {
    objects
        .iter()
        .map(|object| object.as_float().unwrap())
        .collect()
}

#[test]
fn synthetic_fixtures_are_deterministic_and_normalized_models_are_owned() {
    for (name, parts) in support::content_fixtures() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(
            std::fs::read(path).unwrap(),
            zip(&parts, true, false),
            "{name}"
        );
        let parsed = parse(&parts, ParseLimits::default()).unwrap();
        assert_eq!(parsed, fixture(name));
        assert_eq!(parsed.slides().len(), parsed.slide_contents().len());
        if name != "unsupported-style.pptx" {
            assert!(
                parsed.warnings().is_empty(),
                "{name}: {:?}",
                parsed.warnings()
            );
        }
    }
}

#[test]
fn solid_background_is_a_full_page_fill_and_is_painted_first() {
    let model = fixture("solid-background.pptx");
    assert_eq!(
        model.slide_contents()[0].background(),
        Some(Fill::Solid(Color::rgb(48, 64, 80)))
    );
    let display = model.to_display_list();
    let DrawOp::Rect(rect) = display.pages[0].ops[0] else {
        panic!("background")
    };
    assert_eq!(
        rect.rect,
        Rect {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 720.0,
                height: 540.0
            }
        }
    );
    let content = pdf_content(&model).remove(0);
    assert_eq!(operands(&content, "re"), [vec![0.0, 0.0, 720.0, 540.0]]);
    let color = operands(&content, "rg").remove(0);
    for (actual, expected) in color.iter().zip([48.0 / 255.0, 64.0 / 255.0, 80.0 / 255.0]) {
        assert!((actual - expected).abs() < 1e-6);
    }
}

#[test]
fn nontrivial_position_dimensions_and_srgb_fill_reach_pdf() {
    let model = fixture("filled-rectangle.pptx");
    let Shape::Rectangle { rect, fill, stroke } = model.slide_contents()[0].shapes()[0] else {
        panic!("rectangle expected")
    };
    assert_eq!(rect, expected_rect());
    assert_eq!(fill, Some(Fill::Solid(Color::rgb(32, 96, 208))));
    assert_eq!(stroke, None);
    let content = pdf_content(&model).remove(0);
    let coords = operands(&content, "re").remove(0);
    assert_eq!(
        coords,
        [
            rect.origin.x,
            540.0 - rect.origin.y - rect.size.height,
            rect.size.width,
            rect.size.height
        ]
    );
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "f")
            .count(),
        1
    );
    assert!(operands(&content, "RG").is_empty());
    for (actual, channel) in operands(&content, "rg")[0].iter().zip([32, 96, 208]) {
        assert!((*actual - channel as f32 / 255.0).abs() < 1e-6);
    }
}

#[test]
fn fill_and_outline_are_independent_and_width_is_converted_from_emus() {
    let model = fixture("outlined-rectangle.pptx");
    let Shape::Rectangle { fill, stroke, .. } = model.slide_contents()[0].shapes()[0] else {
        panic!("rectangle expected")
    };
    assert_eq!(fill, Some(Fill::Solid(Color::rgb(204, 221, 238))));
    let stroke = stroke.unwrap();
    assert_eq!(stroke.width, 2.5);
    assert_eq!(stroke.color, Color::rgb(208, 32, 48));
    let content = pdf_content(&model).remove(0);
    assert_eq!(operands(&content, "w"), [vec![2.5]]);
    assert_eq!(operands(&content, "re").len(), 2);
    let painted: Vec<_> = content
        .operations
        .iter()
        .filter(|op| matches!(op.operator.as_str(), "f" | "S"))
        .map(|op| op.operator.as_str())
        .collect();
    assert_eq!(painted, ["f", "S"]);
    for (actual, channel) in operands(&content, "RG")[0].iter().zip([208, 32, 48]) {
        assert!((*actual - channel as f32 / 255.0).abs() < 1e-6);
    }
}

#[test]
fn no_fill_preserves_the_background_under_the_outline() {
    let model = fixture("no-fill-rectangle.pptx");
    let Shape::Rectangle { fill, stroke, .. } = model.slide_contents()[0].shapes()[0] else {
        panic!("rectangle expected")
    };
    assert_eq!(fill, Some(Fill::None));
    assert!(stroke.is_some());
    let display = model.to_display_list();
    assert_eq!(display.pages[0].ops.len(), 2);
    assert!(matches!(display.pages[0].ops[0], DrawOp::Rect(_)));
    assert!(matches!(display.pages[0].ops[1], DrawOp::StrokeRect(_)));
    let content = pdf_content(&model).remove(0);
    assert_eq!(operands(&content, "rg").len(), 1); // only background paint
    assert_eq!(operands(&content, "RG").len(), 1);
    let invisible = model_without_fill_or_outline();
    assert_eq!(invisible.slide_contents()[0].shapes().len(), 1);
    assert!(invisible.to_display_list().pages[0].ops.is_empty());
}

fn model_without_fill_or_outline() -> Presentation {
    model(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        ),
    )
}

#[test]
fn overlapping_rectangles_and_negative_positions_keep_xml_paint_order() {
    let model = fixture("multiple-rectangles.pptx");
    assert_eq!(model.slide_contents()[0].shapes().len(), 3);
    let ops = &model.to_display_list().pages[0].ops;
    assert_eq!(ops.len(), 5);
    let colors: Vec<_> = ops
        .iter()
        .map(|op| match op {
            DrawOp::Rect(rect) => rect.color,
            DrawOp::StrokeRect(rect) => rect.color,
            _ => panic!("unexpected operation"),
        })
        .collect();
    assert_eq!(
        colors,
        [
            Color::WHITE,
            Color::rgb(32, 96, 208),
            Color::rgb(240, 176, 32),
            Color::rgb(208, 32, 48),
            Color::rgb(208, 32, 48)
        ]
    );
    let DrawOp::StrokeRect(last) = ops[4] else {
        panic!("outline")
    };
    assert_eq!(last.rect.origin, Point { x: -10.0, y: 20.0 });
    let content = pdf_content(&model).remove(0);
    assert_eq!(
        operands(&content, "re").last().unwrap(),
        &[-10.0, 470.0, 100.0, 50.0]
    );
}

#[test]
fn slide_order_remains_visibly_distinct_after_content_parsing_and_rendering() {
    let model = fixture("reordered-rectangles.pptx");
    assert_eq!(
        model
            .slides()
            .iter()
            .map(|reference| reference.id())
            .collect::<Vec<_>>(),
        [900, 300, 600]
    );
    let content = pdf_content(&model);
    assert_eq!(operands(&content[0], "rg"), [vec![0.0, 0.0, 1.0]]);
    assert_eq!(operands(&content[1], "rg"), [vec![1.0, 0.0, 0.0]]);
    assert_eq!(operands(&content[2], "rg"), [vec![0.0, 1.0, 0.0]]);
}

#[test]
fn unsupported_scheme_colors_and_dashes_warn_without_guessing_paint() {
    let model = fixture("unsupported-style.pptx");
    let Shape::Rectangle { fill, stroke, .. } = model.slide_contents()[0].shapes()[0] else {
        panic!("rectangle expected")
    };
    assert_eq!(fill, None);
    assert_eq!(stroke, None);
    assert!(model.to_display_list().pages[0].ops.is_empty());
    assert!(model.warnings().iter().any(|warning| matches!(warning, PptxWarning::UnsupportedSlideContent { slide_id: 256, part, name } if part == "ppt/slides/slide1.xml" && name.contains("schemeClr"))));
    assert!(
        model
            .warnings()
            .iter()
            .any(|warning| warning.to_string().contains("prstDash"))
    );
}

#[test]
fn unsupported_fills_color_transforms_and_inheritance_warn() {
    for (paint, expected) in [
        ("<a:gradFill/>", "gradFill"),
        ("<a:pattFill/>", "pattFill"),
        (
            "<a:solidFill><a:srgbClr val='123456'><a:alpha val='50000'/></a:srgbClr></a:solidFill>",
            "alpha",
        ),
        (
            "<a:solidFill><a:srgbClr val='123456'><a:tint val='20000'/></a:srgbClr></a:solidFill>",
            "tint",
        ),
        (
            "<a:solidFill><a:sysClr val='window' lastClr='FFFFFF'/></a:solidFill>",
            "sysClr",
        ),
        ("", "inherited fill"),
    ] {
        let model = model(
            "",
            &rectangle(2, NONTRIVIAL_RECT, paint, "<a:ln><a:noFill/></a:ln>"),
        );
        assert!(model.to_display_list().pages[0].ops.is_empty());
        assert!(
            model
                .warnings()
                .iter()
                .any(|warning| warning.to_string().contains(expected)),
            "{expected}"
        );
    }
    let model = model(
        "<p:bg><p:bgRef idx='1001'><a:schemeClr val='bg1'/></p:bgRef></p:bg>",
        "",
    );
    assert!(model.to_display_list().pages[0].ops.is_empty());
    assert!(model.warnings()[0].to_string().contains("bgRef"));
}

#[test]
fn unsupported_shapes_text_styles_and_groups_produce_warnings() {
    let base = rectangle(
        2,
        NONTRIVIAL_RECT,
        &solid("FF0000"),
        "<a:ln><a:noFill/></a:ln>",
    );
    for (shape, warning, count) in [
        (
            base.replace("prst='rect'", "prst='triangle'"),
            "unsupported preset",
            0,
        ),
        (
            base.replace("<a:xfrm>", "<a:xfrm skew='5400000'>"),
            "skew",
            0,
        ),
        (
            base.replace("<a:xfrm>", "<a:xfrm perspective='1'>"),
            "perspective",
            0,
        ),
        (
            base.replace("</p:sp>", "<p:style><a:fillRef idx='1'/></p:style></p:sp>"),
            "style",
            1,
        ),
        (format!("<p:grpSp>{base}</p:grpSp>"), "grpSp", 0),
        ("<p:pic/>".into(), "pic", 0),
        ("<p:graphicFrame/>".into(), "graphicFrame", 0),
        (
            base.replace("<p:nvPr/>", "<p:nvPr><p:ph/></p:nvPr>"),
            "placeholder",
            0,
        ),
    ] {
        let model = model("", &shape);
        assert_eq!(model.slide_contents()[0].shapes().len(), count);
        assert!(
            model
                .warnings()
                .iter()
                .any(|value| value.to_string().contains(warning)),
            "{warning}"
        );
    }
}

#[test]
fn explicit_outline_without_width_uses_documented_base_default_with_warning() {
    let model = model(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            "<a:noFill/>",
            &format!("<a:ln>{}</a:ln>", solid("000000")),
        ),
    );
    let Shape::Rectangle { stroke, .. } = model.slide_contents()[0].shapes()[0] else {
        panic!("rectangle expected")
    };
    assert_eq!(stroke.unwrap().width, 0.75);
    assert!(
        model.warnings()[0]
            .to_string()
            .contains("inherited outline width")
    );
}

#[test]
fn zero_width_outlines_do_not_become_pdf_hairlines() {
    let model = model(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            "<a:noFill/>",
            &format!("<a:ln w='0'>{}</a:ln>", solid("000000")),
        ),
    );
    assert!(model.to_display_list().pages[0].ops.is_empty());
}

#[test]
fn malformed_geometry_color_duplicates_and_xml_return_owned_errors() {
    let base = rectangle(
        2,
        NONTRIVIAL_RECT,
        &solid("FF0000"),
        "<a:ln w='12700'><a:noFill/></a:ln>",
    );
    for shape in [
        base.replace("x='1234567'", "x='NaN'"),
        base.replace("x='1234567'", "x='9223372036854775808'"),
        base.replace("cx='3456789'", "cx='-1'"),
        base.replace("cy='2345678'", "cy='27273042316901'"),
        base.replace("w='12700'", "w='-1'"),
        base.replace("w='12700'", "w='20116801'"),
        base.replace("val='FF0000'", "val='ZZ0000'"),
        base.replace("val='FF0000'", "val='FFF'"),
        base.replace("<a:srgbClr val='FF0000'/>", ""),
        base.replace("<a:off x='1234567' y='765432'/>", ""),
        base.replace("</a:xfrm>", "<a:off x='0' y='0'/></a:xfrm>"),
        base.replace("</p:spPr>", "<a:noFill/></p:spPr>"),
        base.replace("</p:spPr>", "<a:ln/></p:spPr>"),
        base.replace("<a:xfrm>", "<a:xfrm flipH='maybe'>"),
    ] {
        assert!(
            matches!(
                parse(&content_deck("", &shape), ParseLimits::default()),
                Err(PptxError::InvalidSlide { .. })
            ),
            "{shape}"
        );
    }
    let mut parts = content_deck("", &base);
    replace(&mut parts, "ppt/slides/slide1.xml", b"<broken".to_vec());
    assert!(matches!(
        parse(&parts, ParseLimits::default()),
        Err(PptxError::InvalidXml { .. })
    ));
    replace(&mut parts, "ppt/slides/slide1.xml", b"<wrong/>".to_vec());
    assert!(matches!(
        parse(&parts, ParseLimits::default()),
        Err(PptxError::InvalidSlide { .. })
    ));
}

#[test]
fn per_slide_byte_node_depth_limits_and_dtd_rejection_reuse_secure_adapter() {
    for (name, parts) in support::content_error_fixtures() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(std::fs::read(path).unwrap(), zip(&parts, true, false));
        assert!(parse(&parts, ParseLimits::default()).is_err());
    }
    let mut parts = content_deck(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("FF0000"),
            "<a:ln><a:noFill/></a:ln>",
        ),
    );
    for (limits, expected) in [
        (
            ParseLimits {
                max_xml_size: 500,
                ..ParseLimits::default()
            },
            "max_xml_size",
        ),
        (
            ParseLimits {
                max_xml_nodes: 10,
                ..ParseLimits::default()
            },
            "max_xml_nodes",
        ),
        (
            ParseLimits {
                max_xml_depth: 4,
                ..ParseLimits::default()
            },
            "max_xml_depth",
        ),
    ] {
        assert_eq!(
            parse(&parts, limits).unwrap_err(),
            PptxError::LimitExceeded { limit: expected }
        );
    }
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        slide(
            "",
            &format!("{}{}", "<p:ext>".repeat(65), "</p:ext>".repeat(65)),
        ),
    );
    assert_eq!(
        parse(
            &parts,
            ParseLimits {
                max_xml_depth: usize::MAX,
                ..ParseLimits::default()
            }
        )
        .unwrap_err(),
        PptxError::LimitExceeded {
            limit: "max_xml_depth"
        }
    );
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        format!(
            "<!DOCTYPE sld>{}",
            String::from_utf8(slide("", "")).unwrap()
        )
        .into_bytes(),
    );
    assert!(matches!(
        parse(&parts, ParseLimits::default()),
        Err(PptxError::InvalidXml { .. })
    ));
}

#[test]
fn strict_namespaces_utf16_and_alias_prefixes_resolve_slide_semantics() {
    let mut parts = content_deck(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("12aBcD"),
            "<a:ln><a:noFill/></a:ln>",
        ),
    );
    let expected = parse(&parts, ParseLimits::default()).unwrap();
    let text = String::from_utf8(slide(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("12aBcD"),
            "<a:ln><a:noFill/></a:ln>",
        ),
    ))
    .unwrap()
    .replace("<p:", "<pres:")
    .replace("</p:", "</pres:")
    .replace("xmlns:p=", "xmlns:pres=")
    .replace("<a:", "<draw:")
    .replace("</a:", "</draw:")
    .replace("xmlns:a=", "xmlns:draw=");
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        text.as_bytes().to_vec(),
    );
    assert_eq!(parse(&parts, ParseLimits::default()).unwrap(), expected);
    for little in [true, false] {
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        bytes.extend(text.encode_utf16().flat_map(|word| {
            if little {
                word.to_le_bytes()
            } else {
                word.to_be_bytes()
            }
        }));
        replace(&mut parts, "ppt/slides/slide1.xml", bytes);
        assert_eq!(parse(&parts, ParseLimits::default()).unwrap(), expected);
    }
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        text.replace(
            support::PML,
            "http://purl.oclc.org/ooxml/presentationml/main",
        )
        .replace(support::DML, "http://purl.oclc.org/ooxml/drawingml/main")
        .into_bytes(),
    );
    assert_eq!(parse(&parts, ParseLimits::default()).unwrap(), expected);
}

#[test]
fn unreferenced_slides_remain_opaque_and_metadata_only_api_is_preserved() {
    let mut parts = support::deck(3, support::STANDARD_SIZE);
    replace(
        &mut parts,
        "ppt/presentation.xml",
        support::presentation(&format!(
            "{}{}",
            support::ids(&[2, 1]),
            support::STANDARD_SIZE
        )),
    );
    replace(&mut parts, "ppt/slides/slide3.xml", b"not XML".to_vec());
    assert_eq!(
        parse(&parts, ParseLimits::default())
            .unwrap()
            .to_display_list()
            .pages
            .len(),
        2
    );
    replace(&mut parts, "ppt/slides/slide2.xml", b"not XML".to_vec());
    let package =
        OoxmlPackage::from_bytes(&zip(&parts, false, false), SecurityLimits::default()).unwrap();
    let metadata = Presentation::parse(&package, ParseLimits::default()).unwrap();
    assert!(metadata.slide_contents().is_empty());
    assert!(
        metadata
            .to_blank_display_list()
            .pages
            .iter()
            .all(|page| page.ops.is_empty())
    );
    assert!(Presentation::parse_with_content(&package, ParseLimits::default()).is_err());
    assert_eq!(emu_to_points(-12_700), -1.0);
}

#[test]
fn structural_root_group_accepts_empty_or_zero_scaffolding_but_warns_on_transforms() {
    let mut parts = content_deck(
        "",
        &rectangle(
            2,
            NONTRIVIAL_RECT,
            &solid("FF0000"),
            "<a:ln><a:noFill/></a:ln>",
        ),
    );
    let base = String::from_utf8(
        parts
            .iter()
            .find(|(name, _)| name == "ppt/slides/slide1.xml")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    let transform = "<p:grpSpPr><a:xfrm><a:off x='0' y='0'/><a:ext cx='0' cy='0'/><a:chOff x='0' y='0'/><a:chExt cx='0' cy='0'/></a:xfrm></p:grpSpPr>";
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        base.replace("<p:grpSpPr/>", transform).into_bytes(),
    );
    assert_eq!(
        parse(&parts, ParseLimits::default())
            .unwrap()
            .slide_contents()[0]
            .shapes()
            .len(),
        1
    );
    replace(
        &mut parts,
        "ppt/slides/slide1.xml",
        base.replace("<p:grpSpPr/>", &transform.replace("x='0'", "x='1'"))
            .into_bytes(),
    );
    let model = parse(&parts, ParseLimits::default()).unwrap();
    assert!(model.slide_contents()[0].shapes().is_empty());
    assert!(model.warnings()[0].to_string().contains("root group"));
}
