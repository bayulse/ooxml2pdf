mod support;

use lopdf::{Document, content::Content};
use o2p_core::{Color, DrawOp, Point, Rect, Size};
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{Fill, ParseLimits, PptxError, PptxWarning, Presentation, Shape, emu_to_points};
use std::path::Path;
use support::{content_deck, ellipse, outline, solid, straight_line, zip};

fn parse(shapes: &str) -> Result<Presentation, PptxError> {
    let package = OoxmlPackage::from_bytes(
        &zip(&content_deck("", shapes), false, false),
        SecurityLimits::default(),
    )
    .unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default())
}

fn fixture(name: &str) -> Presentation {
    let package = OoxmlPackage::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name),
        SecurityLimits::default(),
    )
    .unwrap();
    Presentation::parse_with_content(&package, ParseLimits::default()).unwrap()
}

fn content(model: &Presentation) -> Content {
    let pdf = Document::load_mem(&o2p_pdf::render(&model.to_display_list()).unwrap()).unwrap();
    Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap()
}

fn operands(content: &Content, operator: &str) -> Vec<Vec<f32>> {
    content
        .operations
        .iter()
        .filter(|op| op.operator == operator)
        .map(|op| op.operands.iter().map(|v| v.as_float().unwrap()).collect())
        .collect()
}

fn paints(content: &Content) -> Vec<&str> {
    content
        .operations
        .iter()
        .map(|op| op.operator.as_str())
        .filter(|op| matches!(*op, "f" | "S"))
        .collect()
}

#[test]
fn all_shape_fixtures_are_small_reproducible_and_warning_free() {
    for (name, parts) in support::shape_fixtures() {
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/pptx")
                .join(name),
        )
        .unwrap();
        assert_eq!(bytes, zip(&parts, true, false), "{name}");
        assert!(bytes.len() < 4096);
        assert!(fixture(name).warnings().is_empty(), "{name}");
    }
}

#[test]
fn ellipse_geometry_fill_and_emu_conversion_reach_display_list_and_pdf() {
    let model = fixture("filled-ellipse.pptx");
    let (x, y, w, h) = support::NONTRIVIAL_RECT;
    let expected = Rect {
        origin: Point {
            x: (x as f64 / 12700.0) as f32,
            y: (y as f64 / 12700.0) as f32,
        },
        size: Size {
            width: (w as f64 / 12700.0) as f32,
            height: (h as f64 / 12700.0) as f32,
        },
    };
    let Shape::Ellipse {
        bounds,
        fill,
        stroke,
    } = model.slide_contents()[0].shapes()[0]
    else {
        panic!("ellipse")
    };
    assert_eq!(bounds, expected);
    assert_eq!(fill, Some(Fill::Solid(Color::rgb(32, 144, 96))));
    assert_eq!(stroke, None);
    let DrawOp::Ellipse(op) = model.to_display_list().pages[0].ops[0] else {
        panic!("ellipse op")
    };
    assert_eq!(op.bounds, expected);
    let pdf = content(&model);
    assert_eq!(operands(&pdf, "c").len(), 4);
    assert_eq!(paints(&pdf), ["f"]);
    assert!(operands(&pdf, "re").is_empty());
}

#[test]
fn ellipse_outline_and_no_fill_are_independent() {
    let model = fixture("outlined-ellipse.pptx");
    let Shape::Ellipse { fill, stroke, .. } = model.slide_contents()[0].shapes()[0] else {
        panic!("ellipse")
    };
    assert_eq!(fill, Some(Fill::Solid(Color::rgb(204, 221, 238))));
    assert_eq!(stroke.unwrap().width, 2.5);
    assert_eq!(stroke.unwrap().color, Color::rgb(208, 32, 48));
    let pdf = content(&model);
    assert_eq!(operands(&pdf, "w"), [vec![2.5]]);
    assert_eq!(paints(&pdf), ["f", "S"]);
    assert_eq!(operands(&pdf, "c").len(), 8);

    let model = fixture("no-fill-ellipse.pptx");
    let Shape::Ellipse { fill, stroke, .. } = model.slide_contents()[0].shapes()[0] else {
        panic!("ellipse")
    };
    assert_eq!(fill, Some(Fill::None));
    assert_eq!(stroke.unwrap().width, 2.0);
    let display = model.to_display_list();
    assert!(matches!(
        display.pages[0].ops.as_slice(),
        [DrawOp::Rect(_), DrawOp::StrokeEllipse(_)]
    ));
    let pdf = content(&model);
    assert_eq!(paints(&pdf), ["f", "S"]); // Background fill, ellipse outline.
    assert_eq!(operands(&pdf, "c").len(), 4);
}

#[test]
fn line_endpoints_color_width_and_top_left_coordinates_reach_pdf() {
    let model = fixture("one-line.pptx");
    assert_eq!(
        model.slide_contents()[0].shapes()[0],
        Shape::Line {
            start: Point { x: 72.0, y: 72.0 },
            end: Point { x: 360.0, y: 216.0 },
            stroke: Some(o2p_pptx::Stroke {
                color: Color::rgb(208, 32, 48),
                width: 1.0
            }),
        }
    );
    let DrawOp::Line(op) = model.to_display_list().pages[0].ops[0] else {
        panic!("line")
    };
    assert_eq!(op.start, Point { x: 72.0, y: 72.0 });
    assert_eq!(op.end, Point { x: 360.0, y: 216.0 });
    let pdf = content(&model);
    assert_eq!(operands(&pdf, "m"), [vec![72.0, 468.0]]);
    assert_eq!(operands(&pdf, "l"), [vec![360.0, 324.0]]);
    assert_eq!(operands(&pdf, "w"), [vec![1.0]]);
    for (actual, channel) in operands(&pdf, "RG")[0].iter().zip([208, 32, 48]) {
        assert!((actual - channel as f32 / 255.0).abs() < 1e-6);
    }
    assert_eq!(paints(&pdf), ["S"]);
    let model = fixture("thick-line.pptx");
    assert_eq!(operands(&content(&model), "w"), [vec![8.0]]);
    let DrawOp::Line(op) = model.to_display_list().pages[0].ops[0] else {
        panic!("line")
    };
    assert_eq!(op.color, Color::rgb(48, 96, 208));
    assert_eq!(op.start.y, op.end.y);
}

#[test]
fn line_flips_handle_both_boolean_spellings_and_signed_positions() {
    let base = straight_line(2, (-12700, -25400, 50800, 76200), &outline("112233", 19050));
    for (attributes, start, end) in [
        ("", (-1.0, -2.0), (3.0, 4.0)),
        (" flipH='1'", (3.0, -2.0), (-1.0, 4.0)),
        (" flipV='true'", (-1.0, 4.0), (3.0, -2.0)),
        (" flipH='true' flipV='1'", (3.0, 4.0), (-1.0, -2.0)),
        (" flipH='false' flipV='0' rot='0'", (-1.0, -2.0), (3.0, 4.0)),
    ] {
        let model = parse(&base.replace("<a:xfrm>", &format!("<a:xfrm{attributes}>"))).unwrap();
        assert!(model.warnings().is_empty());
        let Shape::Line {
            start: a,
            end: b,
            stroke,
        } = model.slide_contents()[0].shapes()[0]
        else {
            panic!("line")
        };
        assert_eq!(
            a,
            Point {
                x: start.0,
                y: start.1
            }
        );
        assert_eq!(b, Point { x: end.0, y: end.1 });
        assert_eq!(stroke.unwrap().width, emu_to_points(19050));
    }
}

#[test]
fn zero_extents_no_stroke_and_zero_width_remain_valid() {
    for coordinates in [(0, 0, 0, 914400), (0, 0, 914400, 0), (0, 0, 0, 0)] {
        let model = parse(&straight_line(2, coordinates, &outline("000000", 12700))).unwrap();
        assert_eq!(paints(&content(&model)), ["S"]);
    }
    for line in ["<a:ln><a:noFill/></a:ln>".to_string(), outline("000000", 0)] {
        let model = parse(&straight_line(2, (0, 0, 12700, 12700), &line)).unwrap();
        assert_eq!(model.slide_contents()[0].shapes().len(), 1);
        assert!(model.to_display_list().pages[0].ops.is_empty());
    }
    let model = parse(&ellipse(
        2,
        (0, 0, 0, 0),
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    ))
    .unwrap();
    assert!(model.to_display_list().pages[0].ops.is_empty());
}

#[test]
fn mixed_shapes_and_overlaps_preserve_xml_z_order_and_rectangle_behavior() {
    for name in ["mixed-shapes.pptx", "overlapping-shapes.pptx"] {
        let model = fixture(name);
        assert!(matches!(
            model.slide_contents()[0].shapes(),
            [
                Shape::Rectangle { .. },
                Shape::Ellipse { .. },
                Shape::Line { .. }
            ]
        ));
        let display = model.to_display_list();
        assert!(matches!(display.pages[0].ops[0], DrawOp::Rect(_)));
        assert!(matches!(display.pages[0].ops[1], DrawOp::Ellipse(_)));
        assert!(matches!(
            display.pages[0].ops.last().unwrap(),
            DrawOp::Line(_)
        ));
        let pdf = content(&model);
        let expected: &[&str] = if name == "mixed-shapes.pptx" {
            &["f", "f", "S", "S"]
        } else {
            &["f", "f", "S"]
        };
        assert_eq!(paints(&pdf), expected);
        assert_eq!(operands(&pdf, "re"), [vec![72.0, 324.0, 216.0, 144.0]]);
        // Distinct fills and stroke prove which overlapping shape paints last.
        let colors: Vec<_> = pdf
            .operations
            .iter()
            .filter(|op| matches!(op.operator.as_str(), "rg" | "RG"))
            .map(|op| (op.operator.as_str(), op.operands[0].as_float().unwrap()))
            .collect();
        assert_eq!(colors[0].0, "rg");
        assert!((colors[0].1 - 48.0 / 255.0).abs() < 1e-6);
        assert!((colors[1].1 - 32.0 / 255.0).abs() < 1e-6);
        assert_eq!(colors.last().unwrap().0, "RG");
        assert!((colors.last().unwrap().1 - 208.0 / 255.0).abs() < 1e-6);
    }
}

#[test]
fn unsupported_ellipse_and_line_styles_produce_slide_scoped_warnings() {
    let base = ellipse(
        2,
        (0, 0, 914400, 914400),
        &solid("FF0000"),
        &outline("000000", 12700),
    );
    for (shape, warning) in [
        (
            base.replace("<a:srgbClr val='FF0000'/>", "<a:schemeClr val='accent1'/>"),
            "schemeClr",
        ),
        (
            base.replace(
                "<a:srgbClr val='FF0000'/>",
                "<a:srgbClr val='FF0000'><a:alpha val='50000'/></a:srgbClr>",
            ),
            "alpha",
        ),
        (
            base.replace("<a:xfrm>", "<a:xfrm rot='5400000'>"),
            "rotation",
        ),
        (
            base.replace(
                "<a:avLst/>",
                "<a:avLst><a:gd name='adj' fmla='val 1'/></a:avLst>",
            ),
            "avLst",
        ),
        (
            base.replace("</a:ln>", "<a:prstDash val='dash'/></a:ln>"),
            "prstDash",
        ),
        (
            base.replace(
                "</p:spPr>",
                "<a:effectLst><a:outerShdw/></a:effectLst></p:spPr>",
            ),
            "effectLst",
        ),
    ] {
        let model = parse(&shape).unwrap();
        assert!(model.warnings().iter().any(|w| matches!(w, PptxWarning::UnsupportedSlideContent { slide_id: 256, part, name } if part == "ppt/slides/slide1.xml" && name.contains(warning))), "{warning}");
    }
    let base = straight_line(2, (0, 0, 914400, 914400), &outline("000000", 12700));
    for (shape, warning) in [
        (
            base.replace("srgbClr val='000000'", "schemeClr val='accent1'"),
            "schemeClr",
        ),
        (
            base.replace("</a:ln>", "<a:headEnd type='triangle'/></a:ln>"),
            "headEnd",
        ),
        (
            base.replace("</a:ln>", "<a:tailEnd type='arrow'/></a:ln>"),
            "tailEnd",
        ),
        (base.replace("<a:ln w=", "<a:ln cap='rnd' w="), "cap"),
        (
            base.replace("prst='line'", "prst='bentConnector3'"),
            "unsupported preset",
        ),
        (
            base.replace("<a:xfrm>", "<a:xfrm rot='5400000'>"),
            "rotation",
        ),
        (
            base.replace(
                "<p:cNvCxnSpPr/>",
                "<p:cNvCxnSpPr><a:stCxn id='2' idx='0'/></p:cNvCxnSpPr>",
            ),
            "stCxn",
        ),
    ] {
        let model = parse(&shape).unwrap();
        assert!(
            model
                .warnings()
                .iter()
                .any(|w| w.to_string().contains(warning)),
            "{warning}"
        );
        if warning != "stCxn" {
            assert!(model.to_display_list().pages[0].ops.is_empty());
        }
    }
    let unknown = parse(&base.replace(&outline("000000", 12700), "")).unwrap();
    assert!(
        unknown
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("inherited outline"))
    );
    let ellipse = parse(
        &base
            .replace("p:cxnSp>", "p:sp>")
            .replace("nvCxnSpPr", "nvSpPr")
            .replace("cNvCxnSpPr", "cNvSpPr")
            .replace("prst='line'", "prst='ellipse'"),
    )
    .unwrap();
    assert!(
        ellipse
            .warnings()
            .iter()
            .any(|w| w.to_string().contains("fill"))
    );
}

#[test]
fn new_geometry_rejects_malformed_supported_values_without_panics() {
    for base in [
        ellipse(
            2,
            (0, 0, 12700, 12700),
            &solid("112233"),
            &outline("000000", 12700),
        ),
        straight_line(2, (0, 0, 12700, 12700), &outline("000000", 12700)),
    ] {
        for shape in [
            base.replace("cx='12700'", "cx='-1'"),
            base.replace("x='0'", "x='overflow'"),
            base.replace("w='12700'", "w='-1'"),
            base.replace("<a:xfrm>", "<a:xfrm flipH='maybe'>"),
            base.replace("<a:off x='0' y='0'/>", ""),
            base.replace("<a:ext cx='12700' cy='12700'/>", ""),
        ] {
            assert!(matches!(parse(&shape), Err(PptxError::InvalidSlide { .. })));
        }
    }
}

#[test]
fn simple_lines_work_as_shapes_in_strict_namespaces_too() {
    let shape = straight_line(2, (0, 0, 12700, 25400), &outline("112233", 12700))
        .replace("p:cxnSp>", "p:sp>")
        .replace("nvCxnSpPr", "nvSpPr")
        .replace("cNvCxnSpPr", "cNvSpPr");
    let mut parts = content_deck("", &shape);
    let slide = parts
        .iter_mut()
        .find(|(name, _)| name == "ppt/slides/slide1.xml")
        .unwrap();
    slide.1 = String::from_utf8(slide.1.clone())
        .unwrap()
        .replace(
            support::PML,
            "http://purl.oclc.org/ooxml/presentationml/main",
        )
        .replace(support::DML, "http://purl.oclc.org/ooxml/drawingml/main")
        .into_bytes();
    let package =
        OoxmlPackage::from_bytes(&zip(&parts, false, false), SecurityLimits::default()).unwrap();
    let model = Presentation::parse_with_content(&package, ParseLimits::default()).unwrap();
    assert!(model.warnings().is_empty());
    assert!(matches!(
        model.slide_contents()[0].shapes(),
        [Shape::Line { .. }]
    ));
    assert_eq!(operands(&content(&model), "l"), [vec![1.0, 538.0]]);
}
