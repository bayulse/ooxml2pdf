use lopdf::{Document, content::Content};
use o2p_core::{Color, DisplayList, DrawOp, EllipseOp, Page, Point, Rect, Size, StrokeEllipseOp};
use o2p_pdf::{PdfError, render};

fn bounds() -> Rect {
    Rect {
        origin: Point { x: 10.0, y: 20.0 },
        size: Size {
            width: 80.0,
            height: 40.0,
        },
    }
}

fn document(bounds: Rect, stroke: bool, width: f32) -> DisplayList {
    let op = if stroke {
        DrawOp::StrokeEllipse(StrokeEllipseOp {
            bounds,
            color: Color::rgb(0, 0, 255),
            width,
        })
    } else {
        DrawOp::Ellipse(EllipseOp {
            bounds,
            color: Color::rgb(255, 0, 0),
        })
    };
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 200.0,
                height: 300.0,
            },
            ops: vec![op],
        }],
    }
}

fn content(document: &DisplayList) -> Content {
    let pdf = Document::load_mem(&render(document).unwrap()).unwrap();
    Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap()
}

#[test]
fn ellipse_quarter_curves_have_correct_geometry_and_isolated_paint() {
    let mut doc = document(bounds(), false, 0.0);
    doc.pages[0]
        .ops
        .push(document(bounds(), true, 2.5).pages[0].ops.remove(0));
    let bytes = render(&doc).unwrap();
    assert_eq!(bytes, render(&doc).unwrap());
    let pdf = content(&doc);
    let operators: Vec<_> = pdf
        .operations
        .iter()
        .map(|op| op.operator.as_str())
        .collect();
    assert_eq!(
        operators,
        [
            "q", "rg", "m", "c", "c", "c", "c", "h", "f", "Q", "q", "RG", "w", "m", "c", "c", "c",
            "c", "h", "S", "Q"
        ]
    );
    let curves: Vec<Vec<f32>> = pdf
        .operations
        .iter()
        .filter(|op| op.operator == "c")
        .map(|op| op.operands.iter().map(|v| v.as_float().unwrap()).collect())
        .collect();
    let expected = [
        [90.0, 248.9543, 72.09139, 240.0, 50.0, 240.0],
        [27.90861, 240.0, 10.0, 248.9543, 10.0, 260.0],
        [10.0, 271.0457, 27.90861, 280.0, 50.0, 280.0],
        [72.09139, 280.0, 90.0, 271.0457, 90.0, 260.0],
    ];
    for (curve, expected) in curves.iter().zip(expected.iter().cycle()) {
        for (actual, expected) in curve.iter().zip(expected) {
            assert!((actual - expected).abs() < 0.0001);
        }
    }
    let moves: Vec<_> = pdf
        .operations
        .iter()
        .filter(|op| op.operator == "m")
        .collect();
    assert_eq!(moves[0].operands[0].as_float().unwrap(), 90.0);
    assert_eq!(moves[0].operands[1].as_float().unwrap(), 260.0);
    let width = pdf.operations.iter().find(|op| op.operator == "w").unwrap();
    assert_eq!(width.operands[0].as_float().unwrap(), 2.5);
    let fill = pdf
        .operations
        .iter()
        .find(|op| op.operator == "rg")
        .unwrap();
    assert_eq!(
        fill.operands
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect::<Vec<_>>(),
        [1.0, 0.0, 0.0]
    );
    let stroke = pdf
        .operations
        .iter()
        .find(|op| op.operator == "RG")
        .unwrap();
    assert_eq!(
        stroke
            .operands
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect::<Vec<_>>(),
        [0.0, 0.0, 1.0]
    );
}

#[test]
fn ellipse_validation_rejects_nonfinite_negative_and_overflowing_geometry() {
    for stroke in [false, true] {
        for invalid in [
            Rect {
                origin: Point {
                    x: f32::NAN,
                    y: 0.0,
                },
                ..bounds()
            },
            Rect {
                origin: Point {
                    x: 0.0,
                    y: f32::INFINITY,
                },
                ..bounds()
            },
            Rect {
                size: Size {
                    width: -1.0,
                    height: 1.0,
                },
                ..bounds()
            },
            Rect {
                size: Size {
                    width: 1.0,
                    height: f32::NAN,
                },
                ..bounds()
            },
            Rect {
                origin: Point {
                    x: f32::MAX,
                    y: 0.0,
                },
                size: Size {
                    width: f32::MAX,
                    height: 1.0,
                },
            },
        ] {
            assert!(matches!(
                render(&document(invalid, stroke, 1.0)),
                Err(PdfError::InvalidGeometry {
                    page: 0,
                    op: Some(0),
                    ..
                })
            ));
        }
        let mut doc = document(
            Rect {
                origin: Point {
                    x: 0.0,
                    y: -f32::MAX,
                },
                size: Size {
                    width: 1.0,
                    height: f32::MAX,
                },
            },
            stroke,
            1.0,
        );
        doc.pages[0].size.height = f32::MAX;
        assert!(matches!(
            render(&doc),
            Err(PdfError::InvalidGeometry { .. })
        ));
    }
    for width in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        assert!(matches!(
            render(&document(bounds(), true, width)),
            Err(PdfError::InvalidGeometry {
                field: "ellipse outline width",
                ..
            })
        ));
    }
}

#[test]
fn zero_size_and_negative_position_ellipses_are_valid() {
    for size in [
        Size {
            width: 0.0,
            height: 40.0,
        },
        Size {
            width: 80.0,
            height: 0.0,
        },
        Size {
            width: 0.0,
            height: 0.0,
        },
    ] {
        for stroke in [false, true] {
            let pdf = content(&document(
                Rect {
                    origin: Point { x: -20.0, y: -10.0 },
                    size,
                },
                stroke,
                1.0,
            ));
            assert_eq!(
                pdf.operations
                    .iter()
                    .filter(|op| op.operator == "c")
                    .count(),
                4
            );
        }
    }
}
