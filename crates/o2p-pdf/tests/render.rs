use lopdf::{Document, Object, content::Content};
use o2p_core::{Color, DisplayList, DrawOp, Font, LineOp, Page, Point, Rect, RectOp, Size, TextOp};
use o2p_pdf::{PdfError, render};

fn fixture() -> DisplayList {
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 200.0,
                height: 300.0,
            },
            ops: vec![
                DrawOp::Text(TextOp {
                    origin: Point { x: 20.0, y: 40.0 },
                    text: "Hello (PDF) \\".into(),
                    font: Font::SansSerif,
                    font_size: 12.0,
                    color: Color::BLACK,
                }),
                DrawOp::Rect(RectOp {
                    rect: Rect {
                        origin: Point { x: 10.0, y: 60.0 },
                        size: Size {
                            width: 50.0,
                            height: 30.0,
                        },
                    },
                    color: Color::rgb(255, 0, 0),
                }),
                DrawOp::Line(LineOp {
                    start: Point { x: 15.0, y: 100.0 },
                    end: Point { x: 90.0, y: 120.0 },
                    width: 2.0,
                    color: Color::rgb(0, 0, 255),
                }),
            ],
        }],
    }
}

fn numbers(objects: &[Object]) -> Vec<f32> {
    objects
        .iter()
        .map(|value| value.as_float().unwrap())
        .collect()
}

#[test]
fn independent_parser_reads_geometry_text_and_paint_order() {
    let bytes = render(&fixture()).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    let pages = pdf.get_pages();
    assert_eq!(pages.len(), 1);
    let page_id = pages[&1];
    let page = pdf.get_dictionary(page_id).unwrap();
    assert_eq!(
        numbers(page.get(b"MediaBox").unwrap().as_array().unwrap()),
        vec![0.0, 0.0, 200.0, 300.0]
    );
    assert_eq!(pdf.extract_text(&[1]).unwrap().trim_end(), "Hello (PDF) \\");
    let (resources, _) = pdf.get_page_resources(page_id).unwrap();
    let fonts = resources.unwrap().get(b"Font").unwrap().as_dict().unwrap();
    let font_id = fonts.get(b"F1").unwrap().as_reference().unwrap();
    assert_eq!(
        pdf.get_dictionary(font_id)
            .unwrap()
            .get(b"BaseFont")
            .unwrap()
            .as_name()
            .unwrap(),
        b"Helvetica"
    );

    let content = Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
    let operators: Vec<_> = content
        .operations
        .iter()
        .map(|op| op.operator.as_str())
        .collect();
    assert_eq!(
        operators,
        vec![
            "q", "rg", "BT", "Tf", "Tm", "Tj", "ET", "Q", "q", "rg", "re", "f", "Q", "q", "RG",
            "w", "m", "l", "S", "Q"
        ]
    );
    let operands = |operator: &str| {
        numbers(
            &content
                .operations
                .iter()
                .find(|op| op.operator == operator)
                .unwrap()
                .operands,
        )
    };
    assert_eq!(operands("Tm"), vec![1.0, 0.0, 0.0, 1.0, 20.0, 260.0]);
    assert_eq!(operands("re"), vec![10.0, 210.0, 50.0, 30.0]);
    assert_eq!(operands("m"), vec![15.0, 200.0]);
    assert_eq!(operands("l"), vec![90.0, 180.0]);
    assert_eq!(operands("w"), vec![2.0]);
    assert_eq!(operands("RG"), vec![0.0, 0.0, 1.0]);
    let fills: Vec<_> = content
        .operations
        .iter()
        .filter(|op| op.operator == "rg")
        .map(|op| numbers(&op.operands))
        .collect();
    assert_eq!(fills, vec![vec![0.0, 0.0, 0.0], vec![1.0, 0.0, 0.0]]);
}

#[test]
fn output_is_byte_for_byte_deterministic() {
    assert_eq!(render(&fixture()).unwrap(), render(&fixture()).unwrap());
}

#[test]
fn multiple_pages_keep_their_own_dimensions_and_coordinate_system() {
    let mut display = fixture();
    let mut second = display.pages[0].clone();
    second.size = Size {
        width: 400.0,
        height: 500.0,
    };
    display.pages.push(second);
    display.pages.push(Page {
        size: Size {
            width: 100.0,
            height: 100.0,
        },
        ops: vec![],
    });
    let pdf = Document::load_mem(&render(&display).unwrap()).unwrap();
    let pages = pdf.get_pages();
    assert_eq!(pages.len(), 3);
    let content = Content::decode(&pdf.get_page_content(pages[&2]).unwrap()).unwrap();
    let matrix = content
        .operations
        .iter()
        .find(|op| op.operator == "Tm")
        .unwrap();
    assert_eq!(
        numbers(&matrix.operands),
        vec![1.0, 0.0, 0.0, 1.0, 20.0, 460.0]
    );
    assert_eq!(
        numbers(
            pdf.get_dictionary(pages[&2])
                .unwrap()
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap()
        ),
        vec![0.0, 0.0, 400.0, 500.0]
    );
    assert!(pdf.get_page_content(pages[&3]).unwrap().is_empty());
}

#[test]
fn invalid_page_sizes_are_rejected() {
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for dimension in [0, 1] {
            let mut display = fixture();
            if dimension == 0 {
                display.pages[0].size.width = invalid;
            } else {
                display.pages[0].size.height = invalid;
            }
            assert_eq!(
                render(&display),
                Err(PdfError::InvalidGeometry {
                    page: 0,
                    op: None,
                    field: "page size"
                })
            );
        }
    }
}

#[test]
fn invalid_operations_report_their_location() {
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut display = fixture();
        let DrawOp::Text(text) = &mut display.pages[0].ops[0] else {
            unreachable!()
        };
        text.font_size = invalid;
        assert_eq!(
            render(&display),
            Err(PdfError::InvalidGeometry {
                page: 0,
                op: Some(0),
                field: "font size"
            })
        );
        let mut display = fixture();
        let DrawOp::Line(line) = &mut display.pages[0].ops[2] else {
            unreachable!()
        };
        line.width = invalid;
        assert_eq!(
            render(&display),
            Err(PdfError::InvalidGeometry {
                page: 0,
                op: Some(2),
                field: "line width"
            })
        );
    }
    let mut display = fixture();
    let DrawOp::Rect(rect) = &mut display.pages[0].ops[1] else {
        unreachable!()
    };
    rect.rect.size.width = -1.0;
    assert_eq!(
        render(&display),
        Err(PdfError::InvalidGeometry {
            page: 0,
            op: Some(1),
            field: "rectangle size"
        })
    );
    let mut display = fixture();
    let DrawOp::Line(line) = &mut display.pages[0].ops[2] else {
        unreachable!()
    };
    line.start.x = f32::NAN;
    assert_eq!(
        render(&display),
        Err(PdfError::InvalidGeometry {
            page: 0,
            op: Some(2),
            field: "line endpoints"
        })
    );
}

#[test]
fn coordinate_conversion_overflow_is_rejected() {
    let mut display = fixture();
    display.pages[0].size.height = f32::MAX;
    let DrawOp::Text(text) = &mut display.pages[0].ops[0] else {
        unreachable!()
    };
    text.origin.y = -f32::MAX;
    assert_eq!(
        render(&display),
        Err(PdfError::InvalidGeometry {
            page: 0,
            op: Some(0),
            field: "text baseline"
        })
    );
}

#[test]
fn rectangle_extent_overflow_is_rejected() {
    let mut display = fixture();
    let DrawOp::Rect(rect) = &mut display.pages[0].ops[1] else {
        unreachable!()
    };
    rect.rect.origin.x = f32::MAX;
    rect.rect.size.width = f32::MAX;
    assert_eq!(
        render(&display),
        Err(PdfError::InvalidGeometry {
            page: 0,
            op: Some(1),
            field: "rectangle position",
        })
    );
}

#[test]
fn invalid_later_page_reports_its_indices() {
    let mut display = fixture();
    let mut second = display.pages[0].clone();
    let DrawOp::Rect(rect) = &mut second.ops[1] else {
        unreachable!()
    };
    rect.rect.origin.y = f32::INFINITY;
    display.pages.push(second);
    assert_eq!(
        render(&display),
        Err(PdfError::InvalidGeometry {
            page: 1,
            op: Some(1),
            field: "rectangle origin",
        })
    );
}

#[test]
fn unicode_and_controls_are_reported_instead_of_replaced() {
    for character in ['é', '中', '\n', '\t', '\0', '\u{7f}'] {
        let mut display = fixture();
        let DrawOp::Text(text) = &mut display.pages[0].ops[0] else {
            unreachable!()
        };
        text.text = format!("before{character}after");
        assert_eq!(
            render(&display),
            Err(PdfError::UnsupportedText {
                page: 0,
                op: 0,
                character
            })
        );
    }
}

#[test]
fn negative_positions_and_zero_sized_rectangles_are_valid() {
    let mut display = fixture();
    let DrawOp::Rect(rect) = &mut display.pages[0].ops[1] else {
        unreachable!()
    };
    rect.rect.origin = Point { x: -10.0, y: -20.0 };
    rect.rect.size = Size {
        width: 0.0,
        height: 0.0,
    };
    Document::load_mem(&render(&display).unwrap()).unwrap();
}

#[test]
fn closed_rectangle_outline_has_isolated_stroke_state_and_no_fill() {
    let mut display = fixture();
    let DrawOp::Rect(rect) = display.pages[0].ops[1] else {
        unreachable!()
    };
    display.pages[0].ops = vec![DrawOp::StrokeRect(o2p_core::StrokeRectOp {
        rect: rect.rect,
        color: Color::rgb(0, 0, 255),
        width: 2.5,
    })];
    let pdf = Document::load_mem(&render(&display).unwrap()).unwrap();
    let content = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    assert_eq!(
        content
            .operations
            .iter()
            .map(|op| op.operator.as_str())
            .collect::<Vec<_>>(),
        ["q", "RG", "w", "re", "S", "Q"]
    );
    assert_eq!(numbers(&content.operations[1].operands), [0.0, 0.0, 1.0]);
    assert_eq!(numbers(&content.operations[2].operands), [2.5]);
    assert_eq!(
        numbers(&content.operations[3].operands),
        [10.0, 210.0, 50.0, 30.0]
    );
}

#[test]
fn rectangle_outline_validates_width_and_geometry_before_rendering() {
    let base = o2p_core::StrokeRectOp {
        rect: Rect {
            origin: Point { x: 10.0, y: 20.0 },
            size: Size {
                width: 30.0,
                height: 40.0,
            },
        },
        color: Color::BLACK,
        width: 1.0,
    };
    for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut display = fixture();
        display.pages[0].ops = vec![DrawOp::StrokeRect(o2p_core::StrokeRectOp { width, ..base })];
        assert_eq!(
            render(&display),
            Err(PdfError::InvalidGeometry {
                page: 0,
                op: Some(0),
                field: "rectangle outline width"
            })
        );
    }
    for (rect, field) in [
        (
            Rect {
                origin: Point {
                    x: f32::NAN,
                    y: 0.0,
                },
                ..base.rect
            },
            "rectangle origin",
        ),
        (
            Rect {
                size: Size {
                    width: -1.0,
                    height: 1.0,
                },
                ..base.rect
            },
            "rectangle size",
        ),
        (
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
            "rectangle position",
        ),
    ] {
        let mut display = fixture();
        display.pages[0].ops = vec![DrawOp::StrokeRect(o2p_core::StrokeRectOp { rect, ..base })];
        assert_eq!(
            render(&display),
            Err(PdfError::InvalidGeometry {
                page: 0,
                op: Some(0),
                field
            })
        );
    }
}
