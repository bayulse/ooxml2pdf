mod support;

use lopdf::{Document, content::Content};
use o2p_core::{DrawOp, Point, Transform};
use o2p_fonts::FontBook;
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{
    ParseLimits, PptxError, PptxWarning, Presentation, Shape, drawingml_rotation_to_degrees,
};
use std::path::PathBuf;
use support::*;

fn parse(parts: &Parts) -> Result<Presentation, PptxError> {
    let package =
        OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default()).unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default())
}

fn fonts() -> FontBook {
    let mut book = FontBook::new();
    book.register_directory(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
        .unwrap();
    book.set_fallback_families(vec![
        "O2P Test Sans".into(),
        "O2P Test Japanese".into(),
        "O2P Test Arabic".into(),
    ]);
    book
}

fn transform(model: &Presentation) -> Transform {
    match &model.slide_contents()[0].shapes()[0] {
        Shape::Transformed { transform, .. } => *transform,
        _ => Transform::IDENTITY,
    }
}

fn assert_point(actual: Point, expected: Point) {
    assert!(
        (actual.x - expected.x).abs() < 0.0001,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual.y - expected.y).abs() < 0.0001,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn drawingml_units_are_centralized_signed_and_normalized_before_float_conversion() {
    for (units, degrees) in [
        (0, 0.0),
        (2_700_000, 45.0),
        (5_400_000, 90.0),
        (10_800_000, 180.0),
        (-5_400_000, 270.0),
        (21_600_000, 0.0),
        (24_300_000, 45.0),
        (-24_300_000, 315.0),
    ] {
        assert_eq!(drawingml_rotation_to_degrees(units), degrees);
    }
    assert!((drawingml_rotation_to_degrees(60_001) - 1.0000167).abs() < 1e-7);
    assert!((0.0..360.0).contains(&drawingml_rotation_to_degrees(i32::MIN)));
    assert!((0.0..360.0).contains(&drawingml_rotation_to_degrees(i32::MAX)));
}

#[test]
fn parser_preserves_bounds_and_rotates_around_nonzero_element_center() {
    let base = rectangle(
        2,
        (127000, 254000, 508000, 254000),
        &solid("123456"),
        &outline("000000", 12700),
    );
    let center = Point { x: 30.0, y: 30.0 };
    for (units, expected) in [
        ("0", Point { x: 10.0, y: 20.0 }),
        (
            "2700000",
            Point {
                x: 22.928932,
                y: 8.786797,
            },
        ),
        ("5400000", Point { x: 40.0, y: 10.0 }),
        ("10800000", Point { x: 50.0, y: 40.0 }),
        ("-5400000", Point { x: 20.0, y: 50.0 }),
        ("21600000", Point { x: 10.0, y: 20.0 }),
    ] {
        let model = parse(&content_deck(
            "",
            &base.replace("<a:xfrm>", &format!("<a:xfrm rot='{units}'>")),
        ))
        .unwrap();
        assert!(model.warnings().is_empty());
        let t = transform(&model);
        assert_point(t.apply(center), center);
        assert_point(t.apply(Point { x: 10.0, y: 20.0 }), expected);
        let shape = match &model.slide_contents()[0].shapes()[0] {
            Shape::Transformed { element, .. } => &element[0],
            shape => shape,
        };
        let Shape::Rectangle { rect, .. } = shape else {
            panic!("rectangle")
        };
        assert_eq!(rect.origin, Point { x: 10.0, y: 20.0 });
        assert_eq!((rect.size.width, rect.size.height), (40.0, 20.0));
    }
}

#[test]
fn local_flip_axes_precede_rotation_for_shapes_lines_and_pictures() {
    let coords = (127000, 254000, 508000, 254000);
    let center = Point { x: 30.0, y: 30.0 };
    for (attrs, expected) in [
        ("flipH='1'", Point { x: 50.0, y: 20.0 }),
        ("flipV='true'", Point { x: 10.0, y: 40.0 }),
        ("flipH='true' flipV='1'", Point { x: 50.0, y: 40.0 }),
        ("rot='5400000' flipH='1'", Point { x: 40.0, y: 50.0 }),
        ("rot='5400000' flipV='1'", Point { x: 20.0, y: 10.0 }),
        (
            "rot='5400000' flipH='1' flipV='1'",
            Point { x: 20.0, y: 50.0 },
        ),
    ] {
        for (base, picture) in [
            (
                rectangle(2, coords, &solid("123456"), &outline("000000", 12700)),
                false,
            ),
            (
                ellipse(2, coords, &solid("123456"), &outline("000000", 12700)),
                false,
            ),
            (straight_line(2, coords, &outline("000000", 12700)), false),
            (self::picture(2, coords), true),
        ] {
            let xml = base.replace("<a:xfrm>", &format!("<a:xfrm {attrs}>"));
            let model = parse(&if picture {
                image_deck(false, &xml)
            } else {
                content_deck("", &xml)
            })
            .unwrap();
            assert!(model.warnings().is_empty(), "{attrs}");
            assert_point(transform(&model).apply(center), center);
            assert_point(
                transform(&model).apply(Point { x: 10.0, y: 20.0 }),
                expected,
            );
        }
    }
}

#[test]
fn transform_fixtures_are_reproducible_and_pdf_uses_native_paints_in_xml_order() {
    let fonts = fonts();
    for (name, parts) in transform_fixtures() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(bytes, zip(&parts, true, false), "{name}");
        assert!(bytes.len() < 8192);
        let model = parse(&parts).unwrap();
        assert!(
            model.warnings().iter().all(|w| {
                let message = w.to_string();
                message.contains("text inheritance/font resolution")
                    || message.contains("legacy text box overflow")
            }),
            "{name}: {:?}",
            model.warnings()
        );
        let display = model
            .to_display_list_with_fonts(&fonts)
            .unwrap()
            .display_list;
        let bytes = o2p_pdf::render(&display).unwrap();
        assert_eq!(o2p_pdf::render(&display).unwrap(), bytes);
        let pdf = Document::load_mem(&bytes).unwrap();
        let content = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
        let names: Vec<_> = content
            .operations
            .iter()
            .map(|o| o.operator.as_str())
            .collect();
        let paints: Vec<_> = names
            .iter()
            .copied()
            .filter(|o| matches!(*o, "f" | "S" | "Do"))
            .collect();
        assert!(names.contains(&"cm"), "{name}");
        let starts = display.pages[0]
            .ops
            .iter()
            .filter(|o| matches!(o, DrawOp::BeginTransform(_)))
            .count();
        let ends = display.pages[0]
            .ops
            .iter()
            .filter(|o| matches!(o, DrawOp::EndTransform))
            .count();
        assert_eq!(starts, ends);
        if name.contains("ellipse") {
            assert_eq!(names.iter().filter(|&&o| o == "c").count(), 8);
            assert_eq!(paints, ["f", "S"]);
        } else if name.contains("image") {
            assert!(names.contains(&"Do"));
            assert!(!names.contains(&"Tj"));
        } else if name.contains("rectangle") {
            assert_eq!(paints, ["f", "S"]);
            assert_eq!(names.iter().filter(|&&o| o == "re").count(), 2);
        } else if name == "mixed-transforms.pptx" {
            assert_eq!(starts, 9);
            assert_eq!(
                paints,
                [
                    "f", "f", "S", "f", "S", "f", "S", "Do", "f", "S", "f", "S", "f", "S", "Do",
                    "S"
                ]
            );
            // Two transformed instances still share one original raster resource.
            assert_eq!(
                pdf.objects
                    .values()
                    .filter_map(|o| o.as_stream().ok())
                    .filter(
                        |s| s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok())
                            == Some(b"Image".as_slice())
                    )
                    .count(),
                1
            );
        }
    }
}

#[test]
fn geometry_text_unicode_fonts_and_wrapping_share_one_scope_without_relayout() {
    let parts = transform_fixtures()
        .into_iter()
        .find(|(n, _)| *n == "rotated-text.pptx")
        .unwrap()
        .1;
    let transformed = parse(&parts).unwrap();
    let mut plain = parts;
    let xml = String::from_utf8(
        plain
            .iter()
            .find(|(n, _)| n == "ppt/slides/slide1.xml")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap()
    .replace(" rot='2700000'", "");
    replace(&mut plain, "ppt/slides/slide1.xml", xml.into_bytes());
    let plain = parse(&plain).unwrap();
    let fonts = fonts();
    let rotated = transformed.to_display_list_with_fonts(&fonts).unwrap();
    let original = plain.to_display_list_with_fonts(&fonts).unwrap();
    assert_eq!(rotated.warnings, original.warnings);
    let ops = &rotated.display_list.pages[0].ops;
    assert!(matches!(ops[0], DrawOp::BeginTransform(_)));
    assert!(matches!(ops.last(), Some(DrawOp::EndTransform)));
    assert_eq!(&ops[1..ops.len() - 1], original.display_list.pages[0].ops);
    assert!(matches!(ops[1], DrawOp::Rect(_)) && matches!(ops[2], DrawOp::StrokeRect(_)));
    let texts: Vec<_> = ops
        .iter()
        .filter_map(|o| {
            if let DrawOp::GlyphText(t) = o {
                Some(t)
            } else {
                None
            }
        })
        .collect();
    let baselines: std::collections::BTreeSet<_> =
        texts.iter().map(|t| t.origin.y.to_bits()).collect();
    assert!(baselines.len() >= 2, "must exercise measured wrapping");
    let source: String = texts.iter().map(|t| t.text.source.as_str()).collect();
    assert_eq!(source, "Résumé 日本語 ffi wraps with real selectable text.");
    let bytes = o2p_pdf::render(&rotated.display_list).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert!(extracted.contains("Résumé") && extracted.contains("日本語"));
    assert!(
        pdf.objects
            .values()
            .any(|o| o.as_dict().is_ok_and(|d| d.has(b"FontFile2")))
    );
    assert!(
        pdf.objects
            .values()
            .filter(|o| o.as_dict().is_ok_and(|d| d.has(b"ToUnicode")))
            .count()
            >= 2
    );
    let raw = pdf.get_page_content(pdf.get_pages()[&1]).unwrap();
    assert!(String::from_utf8_lossy(&raw).contains("/ActualText"));
    let content = Content::decode(&raw).unwrap();
    let mut transformed_scope = false;
    let mut depth = 0;
    for op in content.operations {
        match op.operator.as_str() {
            "q" => depth += 1,
            "Q" => {
                depth -= 1;
                if depth == 0 {
                    transformed_scope = false;
                }
            }
            "cm" => transformed_scope = true,
            "f" | "S" | "Tj" => assert!(transformed_scope && depth >= 2),
            _ => {}
        }
    }
    assert_eq!(depth, 0);
}

#[test]
fn unsupported_transforms_warn_and_malformed_values_fail_without_partial_elements() {
    let base = rectangle(
        2,
        (127000, 254000, 508000, 254000),
        &solid("123456"),
        &outline("000000", 12700),
    );
    for xml in [
        base.replace("<a:xfrm>", "<a:xfrm skew='10'>"),
        base.replace("</a:xfrm>", "<a:chOff x='0' y='0'/></a:xfrm>"),
        base.replace("</p:spPr>", "<a:scene3d/></p:spPr>"),
        format!("<p:grpSp>{base}</p:grpSp>"),
    ] {
        let model = parse(&content_deck("", &xml)).unwrap();
        assert!(model.slide_contents()[0].shapes().is_empty());
        assert!(model.warnings().iter().any(|w|matches!(w,PptxWarning::UnsupportedSlideContent{slide_id:256,part,..} if part=="ppt/slides/slide1.xml")));
    }
    for attrs in [
        "rot='bad'",
        "rot='2147483648'",
        "rot='-2147483649'",
        "flipH='bad'",
        "flipV='2'",
    ] {
        let xml = base.replace("<a:xfrm>", &format!("<a:xfrm {attrs}>"));
        assert!(matches!(
            parse(&content_deck("", &xml)),
            Err(PptxError::InvalidSlide { .. })
        ));
    }
}

#[test]
fn identity_transforms_retain_previous_output_and_strict_namespaces_share_the_adapter() {
    for base in [
        rectangle(
            2,
            (127000, 254000, 508000, 254000),
            &solid("123456"),
            &outline("000000", 12700),
        ),
        ellipse(
            2,
            (127000, 254000, 508000, 254000),
            &solid("123456"),
            &outline("000000", 12700),
        ),
        straight_line(
            2,
            (127000, 254000, 508000, 254000),
            &outline("000000", 12700),
        ),
    ] {
        let original = parse(&content_deck("", &base)).unwrap();
        let explicit = parse(&content_deck(
            "",
            &base.replace(
                "<a:xfrm>",
                "<a:xfrm rot='21600000' flipH='false' flipV='0'>",
            ),
        ))
        .unwrap();
        assert_eq!(original, explicit);
        assert_eq!(
            o2p_pdf::render(&original.to_display_list()).unwrap(),
            o2p_pdf::render(&explicit.to_display_list()).unwrap()
        );
    }
    let xml = picture(2, (127000, 254000, 508000, 254000))
        .replace("<a:xfrm>", "<a:xfrm rot='5400000' flipH='1'>");
    let original = image_deck(true, &xml);
    let transitional = parse(&original).unwrap();
    let mut strict = original;
    let slide = &mut strict
        .iter_mut()
        .find(|(name, _)| name == "ppt/slides/slide1.xml")
        .unwrap()
        .1;
    *slide = String::from_utf8(slide.clone())
        .unwrap()
        .replace(PML, "http://purl.oclc.org/ooxml/presentationml/main")
        .replace(DML, "http://purl.oclc.org/ooxml/drawingml/main")
        .replace(
            OFFICE_RELS,
            "http://purl.oclc.org/ooxml/officeDocument/relationships",
        )
        .into_bytes();
    let strict = parse(&strict).unwrap();
    assert_eq!(transitional.to_display_list(), strict.to_display_list());
    assert_eq!(
        o2p_pdf::render(&transitional.to_display_list()).unwrap(),
        o2p_pdf::render(&strict.to_display_list()).unwrap()
    );
}

#[test]
fn transformed_overlaps_keep_xml_z_order_across_fill_image_and_text() {
    let rect = rectangle(
        2,
        (127000, 254000, 508000, 254000),
        &solid("123456"),
        &outline("000000", 12700),
    )
    .replace("<a:xfrm>", "<a:xfrm rot='2700000'>");
    let image =
        picture(3, (127000, 254000, 508000, 254000)).replace("<a:xfrm>", "<a:xfrm rot='5400000'>");
    let text = text_shape(
        4,
        (127000, 254000, 508000, 254000),
        "<a:p><a:r><a:rPr sz='1200'/><a:t>Top</a:t></a:r></a:p>",
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    )
    .replace("<a:xfrm>", "<a:xfrm flipH='1'>");
    for (xml, expected) in [
        (
            format!("{rect}{image}{text}"),
            vec!["f", "f", "S", "Do", "Tj"],
        ),
        (
            format!("{image}{text}{rect}"),
            vec!["f", "Do", "Tj", "f", "S"],
        ),
    ] {
        let model = parse(&image_deck(false, &xml)).unwrap();
        let bytes = o2p_pdf::render(&model.to_display_list()).unwrap();
        let pdf = Document::load_mem(&bytes).unwrap();
        let stream = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
        let paints: Vec<_> = stream
            .operations
            .iter()
            .map(|o| o.operator.as_str())
            .filter(|o| matches!(*o, "f" | "S" | "Do" | "Tj"))
            .collect();
        assert_eq!(paints, expected);
    }
}
