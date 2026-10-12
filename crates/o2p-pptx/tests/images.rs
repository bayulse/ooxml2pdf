mod support;

use o2p_core::{DrawOp, ImageFormat, Point, Size};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{ParseLimits, PptxError, PptxWarning, Presentation, Shape};
use std::{path::Path, sync::Arc};
use support::{image_deck, picture, replace, zip};

fn parse(parts: &support::Parts) -> Result<Presentation, PptxError> {
    let package = OoxmlPackage::from_bytes(&zip(parts, false, false), SecurityLimits::default())?;
    Presentation::parse_with_content(&package, ParseLimits::default())
}

fn image(model: &Presentation) -> &o2p_core::ImageOp {
    let Shape::Image(image) = &model.slide_contents()[0].shapes()[0] else {
        panic!("picture")
    };
    image
}

fn mutate(parts: &mut support::Parts, name: &str, from: &str, to: &str) {
    let bytes = parts
        .iter()
        .find(|(part, _)| part == name)
        .unwrap()
        .1
        .clone();
    replace(
        parts,
        name,
        String::from_utf8(bytes)
            .unwrap()
            .replace(from, to)
            .into_bytes(),
    );
}

#[test]
fn six_fixtures_are_reproducible_owned_warning_free_and_renderable() {
    for (name, parts) in support::image_fixtures() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            zip(&parts, true, false),
            "{name}"
        );
        let model = parse(&parts).unwrap();
        assert!(
            model
                .warnings()
                .iter()
                .all(support::is_missing_layout_warning),
            "{name}: {:?}",
            model.warnings()
        );
        let package = OoxmlPackage::open(path, SecurityLimits::default()).unwrap();
        assert_eq!(
            model,
            Presentation::parse_with_content(&package, ParseLimits::default()).unwrap()
        );
        let bytes = o2p_pdf::render(&model.to_display_list()).unwrap();
        assert_eq!(bytes, o2p_pdf::render(&model.to_display_list()).unwrap());
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let images: Vec<_> = pdf
            .objects
            .values()
            .filter_map(|object| object.as_stream().ok())
            .filter(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|v| v.as_name().ok())
                    == Some(b"Image".as_slice())
            })
            .collect();
        assert_eq!(images.len(), 1, "{name}");
        let jpeg = name == "one-jpeg.pptx";
        assert_eq!(
            images[0].dict.get(b"Width").unwrap().as_i64().unwrap(),
            if jpeg { 3 } else { 2 }
        );
        assert_eq!(images[0].dict.get(b"Height").unwrap().as_i64().unwrap(), 2);
        let pixels = images[0].decompressed_content().unwrap();
        if jpeg {
            for pixel in pixels.chunks_exact(3) {
                for (actual, expected) in pixel.iter().zip([20i16, 160, 60]) {
                    assert!((i16::from(*actual) - expected).abs() <= 3);
                }
            }
        } else {
            assert_eq!(pixels, [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]);
        }
    }
}

#[test]
fn signatures_positions_dimensions_and_scaling_use_central_emu_conversion() {
    for (jpeg, coords, expected, format) in [
        (
            false,
            (-127_000, 254_000, 3_657_600, 914_400),
            (
                Point { x: -10.0, y: 20.0 },
                Size {
                    width: 288.0,
                    height: 72.0,
                },
            ),
            ImageFormat::Png,
        ),
        (
            true,
            (914_400, 914_400, 2_743_200, 1_828_800),
            (
                Point { x: 72.0, y: 72.0 },
                Size {
                    width: 216.0,
                    height: 144.0,
                },
            ),
            ImageFormat::Jpeg,
        ),
    ] {
        let model = parse(&image_deck(jpeg, &picture(2, coords))).unwrap();
        let picture = image(&model);
        assert_eq!(picture.rect.origin, expected.0);
        assert_eq!(picture.rect.size, expected.1);
        assert_eq!(picture.image.format, format);
        let display = model.to_display_list();
        let DrawOp::Image(op) = &display.pages[0].ops[1] else {
            panic!("image after background")
        };
        assert!(Arc::ptr_eq(&op.image, &picture.image));
        let pdf = lopdf::Document::load_mem(&o2p_pdf::render(&display).unwrap()).unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        let matrix = &content
            .operations
            .iter()
            .find(|op| op.operator == "cm")
            .unwrap()
            .operands;
        assert_eq!(
            matrix
                .iter()
                .map(|v| v.as_float().unwrap())
                .collect::<Vec<_>>(),
            [
                expected.1.width,
                0.0,
                0.0,
                expected.1.height,
                expected.0.x,
                540.0 - expected.0.y - expected.1.height
            ]
        );
    }
    let coords = support::NONTRIVIAL_RECT;
    let model = parse(&image_deck(false, &picture(2, coords))).unwrap();
    let rect = image(&model).rect;
    assert_eq!(
        [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height
        ],
        [coords.0, coords.1, coords.2, coords.3].map(|emu| (emu as f64 / 12700.0) as f32)
    );
}

#[test]
fn repeated_media_is_shared_across_pictures_models_and_display_lists() {
    let model = parse(&image_deck(
        false,
        &format!(
            "{}{}",
            picture(2, (0, 0, 12700, 12700)),
            picture(3, (12700, 0, 12700, 12700))
        ),
    ))
    .unwrap();
    let Shape::Image(second) = &model.slide_contents()[0].shapes()[1] else {
        panic!("picture")
    };
    assert!(Arc::ptr_eq(&image(&model).image, &second.image));
    assert!(Arc::ptr_eq(
        &image(&model).image,
        &image(&model.clone()).image
    ));
    let pdf =
        lopdf::Document::load_mem(&o2p_pdf::render(&model.to_display_list()).unwrap()).unwrap();
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
    let resources = pdf
        .get_page_resources(pdf.get_pages()[&1])
        .unwrap()
        .0
        .unwrap();
    assert_eq!(
        resources.get(b"XObject").unwrap().as_dict().unwrap().len(),
        1
    );
}

#[test]
fn missing_wrong_and_external_image_relationships_are_explicit() {
    let base = image_deck(false, &picture(2, (0, 0, 12700, 12700)));
    for (from, to) in [
        ("r:embed='rImage'", "r:embed='missing'"),
        ("r:embed='rImage'", "r:embed=''"),
        ("r:embed='rImage'", ""),
    ] {
        let mut parts = base.clone();
        mutate(&mut parts, "ppt/slides/slide1.xml", from, to);
        assert!(matches!(parse(&parts), Err(PptxError::InvalidSlide { .. })));
    }
    let mut parts = base.clone();
    mutate(
        &mut parts,
        "ppt/slides/_rels/slide1.xml.rels",
        "/image'",
        "/hyperlink'",
    );
    assert!(matches!(parse(&parts), Err(PptxError::InvalidSlide { .. })));
    let mut parts = base;
    mutate(
        &mut parts,
        "ppt/slides/_rels/slide1.xml.rels",
        "Target='../media/synthetic.png'",
        "Target='https://example.invalid/image.png' TargetMode='External'",
    );
    let model = parse(&parts).unwrap();
    assert!(model.slide_contents()[0].shapes().is_empty());
    assert!(
        model
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("never fetched"))
    );
}

#[test]
fn strict_namespaces_and_canonical_relative_targets_are_supported() {
    let mut parts = image_deck(false, &picture(2, (0, 0, 12700, 12700)));
    mutate(
        &mut parts,
        "ppt/slides/slide1.xml",
        support::PML,
        "http://purl.oclc.org/ooxml/presentationml/main",
    );
    mutate(
        &mut parts,
        "ppt/slides/slide1.xml",
        support::DML,
        "http://purl.oclc.org/ooxml/drawingml/main",
    );
    mutate(
        &mut parts,
        "ppt/slides/slide1.xml",
        support::OFFICE_RELS,
        "http://purl.oclc.org/ooxml/officeDocument/relationships",
    );
    mutate(
        &mut parts,
        "ppt/slides/_rels/slide1.xml.rels",
        support::OFFICE_RELS,
        "http://purl.oclc.org/ooxml/officeDocument/relationships",
    );
    mutate(
        &mut parts,
        "ppt/slides/_rels/slide1.xml.rels",
        "../media/synthetic.png",
        "../media/./synthetic.png",
    );
    assert_eq!(
        image(&parse(&parts).unwrap()).image.format,
        ImageFormat::Png
    );
}

#[test]
fn unsupported_formats_and_picture_behaviors_produce_structured_warnings() {
    let base = image_deck(false, &picture(2, (0, 0, 12700, 12700)));
    let mut unsupported = base.clone();
    mutate(
        &mut unsupported,
        "[Content_Types].xml",
        "image/png",
        "image/gif",
    );
    replace(
        &mut unsupported,
        "ppt/media/synthetic.png",
        b"GIF89a unsupported".to_vec(),
    );
    let model = parse(&unsupported).unwrap();
    assert!(model.slide_contents()[0].shapes().is_empty());
    assert!(model.warnings().iter().any(|w|matches!(w,PptxWarning::UnsupportedSlideContent{slide_id:256,part,name} if part=="ppt/slides/slide1.xml" && name.contains("image/gif"))));
    for (from, to, diagnostic) in [
        (
            "<a:stretch>",
            "<a:srcRect l='10000'/><a:stretch>",
            "srcRect",
        ),
        ("<a:stretch><a:fillRect/></a:stretch>", "<a:tile/>", "tile"),
        ("<a:fillRect/>", "<a:fillRect l='10000'/>", "stretch"),
        ("<a:xfrm>", "<a:xfrm skew='60000'>", "skew"),
        ("<a:xfrm>", "<a:xfrm perspective='1'>", "perspective"),
        (
            "r:embed='rImage'/>",
            "r:embed='rImage'><a:alphaModFix amt='50000'/></a:blip>",
            "alphaModFix",
        ),
        (
            "</p:spPr>",
            "<a:effectLst><a:outerShdw/></a:effectLst></p:spPr>",
            "effectLst",
        ),
    ] {
        let mut parts = base.clone();
        mutate(&mut parts, "ppt/slides/slide1.xml", from, to);
        let model = parse(&parts).unwrap();
        assert!(
            model.slide_contents()[0].shapes().is_empty(),
            "{diagnostic}"
        );
        assert!(
            model
                .warnings()
                .iter()
                .any(|w| w.to_string().contains(diagnostic)),
            "{diagnostic}"
        );
    }
}

#[test]
fn malformed_media_fails_without_panics_or_partial_pdf() {
    for malformed in [b"not a PNG".to_vec(), b"\x89PNG\r\n\x1a\n".to_vec()] {
        let mut parts = image_deck(false, &picture(2, (0, 0, 12700, 12700)));
        replace(&mut parts, "ppt/media/synthetic.png", malformed);
        match parse(&parts) {
            Err(PptxError::InvalidSlide { .. }) => {}
            Ok(model) => assert!(matches!(
                o2p_pdf::render(&model.to_display_list()),
                Err(o2p_pdf::PdfError::InvalidImage { page: 0, .. })
            )),
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn image_and_shapes_preserve_xml_z_order_in_normalized_core_and_pdf_models() {
    for (name, parts) in support::image_fixtures()
        .into_iter()
        .filter(|(name, _)| name.contains("shape"))
    {
        let model = parse(&parts).unwrap();
        let ops = &model.to_display_list().pages[0].ops;
        let expected = if name == "image-with-shapes.pptx" {
            vec!["f", "f", "Do", "f", "S"]
        } else {
            vec!["f", "Do", "f"]
        };
        if name == "image-with-shapes.pptx" {
            assert!(matches!(
                &model.slide_contents()[0].shapes(),
                [
                    Shape::Rectangle { .. },
                    Shape::Image(_),
                    Shape::Ellipse { .. },
                    Shape::Line { .. }
                ]
            ));
            assert!(matches!(
                ops.as_slice(),
                [
                    DrawOp::Rect(_),
                    DrawOp::Rect(_),
                    DrawOp::Image(_),
                    DrawOp::Ellipse(_),
                    DrawOp::Line(_)
                ]
            ));
        } else {
            assert!(matches!(
                ops.as_slice(),
                [DrawOp::Rect(_), DrawOp::Image(_), DrawOp::Rect(_)]
            ));
        }
        let pdf =
            lopdf::Document::load_mem(&o2p_pdf::render(&model.to_display_list()).unwrap()).unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        assert_eq!(
            content
                .operations
                .iter()
                .map(|op| op.operator.as_str())
                .filter(|op| matches!(*op, "f" | "S" | "Do"))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn malformed_picture_metadata_errors_and_inherited_placement_warns() {
    let base = image_deck(false, &picture(2, (0, 0, 12700, 12700)));
    for (from, to) in [
        (
            "<a:stretch><a:fillRect/></a:stretch>",
            "<a:stretch><a:fillRect/></a:stretch><a:stretch><a:fillRect/></a:stretch>",
        ),
        ("<a:off x='0' y='0'/>", ""),
        ("cx='12700'", "cx='-1'"),
        ("<a:xfrm>", "<a:xfrm flipH='bad'>"),
        ("<a:xfrm>", "<a:xfrm rot='bad'>"),
        ("x='0'", "x='9999999999999999999999999999999999999'"),
        (
            "<a:ext cx='12700' cy='12700'/>",
            "<a:ext cx='12700' cy='12700'/><a:ext cx='12700' cy='12700'/>",
        ),
    ] {
        let mut parts = base.clone();
        mutate(&mut parts, "ppt/slides/slide1.xml", from, to);
        assert!(
            matches!(parse(&parts), Err(PptxError::InvalidSlide { .. })),
            "{to}"
        );
    }
    let mut parts = base;
    mutate(
        &mut parts,
        "ppt/slides/slide1.xml",
        "<a:xfrm><a:off x='0' y='0'/><a:ext cx='12700' cy='12700'/></a:xfrm>",
        "",
    );
    let model = parse(&parts).unwrap();
    assert!(model.slide_contents()[0].shapes().is_empty());
    assert!(
        model
            .warnings()
            .iter()
            .any(|warning| warning.to_string().contains("inherited picture transform"))
    );
}
