use lopdf::{Document, content::Content};
use o2p_core::{
    Color, DisplayList, DrawOp, Font, Page, Point, Rect, RectOp, Size, TextOp, Transform,
};
use o2p_pdf::{PdfError, render};

fn display(ops: Vec<DrawOp>) -> DisplayList {
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 200.0,
                height: 300.0,
            },
            ops,
        }],
    }
}

fn rectangle() -> DrawOp {
    DrawOp::Rect(RectOp {
        rect: Rect {
            origin: Point { x: 10.0, y: 40.0 },
            size: Size {
                width: 40.0,
                height: 20.0,
            },
        },
        color: Color::BLACK,
    })
}

#[test]
fn backend_conjugates_generic_rotation_and_isolates_following_element() {
    let transform =
        Transform::rotation_and_flips(90.0, true, false).around(Point { x: 30.0, y: 50.0 });
    let input = display(vec![
        DrawOp::BeginTransform(transform),
        rectangle(),
        DrawOp::Text(TextOp {
            origin: Point { x: 12.0, y: 52.0 },
            text: "Selectable".into(),
            font: Font::SansSerif,
            font_size: 10.0,
            color: Color::BLACK,
        }),
        DrawOp::EndTransform,
        rectangle(),
    ]);
    let bytes = render(&input).unwrap();
    assert_eq!(render(&input).unwrap(), bytes);
    let pdf = Document::load_mem(&bytes).unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains("Selectable"));
    let stream = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    let matrix: Vec<_> = stream
        .operations
        .iter()
        .find(|o| o.operator == "cm")
        .unwrap()
        .operands
        .iter()
        .map(|v| v.as_float().unwrap())
        .collect();
    // P * [0,-1,-1,0,80,80] * P, for P(x,y)=(x,300-y).
    assert_eq!(matrix, [0.0, 1.0, 1.0, 0.0, -220.0, 220.0]);
    let mut depth = 0;
    let mut painted = vec![];
    for op in &stream.operations {
        match op.operator.as_str() {
            "q" => depth += 1,
            "Q" => depth -= 1,
            "f" | "Tj" => painted.push((op.operator.as_str(), depth)),
            _ => {}
        }
        assert!(depth >= 0);
    }
    assert_eq!(depth, 0);
    assert_eq!(painted, [("f", 2), ("Tj", 2), ("f", 1)]);
}

#[test]
fn translation_is_not_confused_with_pdf_y_direction() {
    let pdf = Document::load_mem(
        &render(&display(vec![
            DrawOp::BeginTransform(Transform::translation(Point { x: 5.0, y: 7.0 })),
            rectangle(),
            DrawOp::EndTransform,
        ]))
        .unwrap(),
    )
    .unwrap();
    let stream = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    let matrix: Vec<_> = stream
        .operations
        .iter()
        .find(|o| o.operator == "cm")
        .unwrap()
        .operands
        .iter()
        .map(|v| v.as_float().unwrap())
        .collect();
    assert_eq!(matrix, [1.0, 0.0, 0.0, 1.0, 5.0, -7.0]);
}

#[test]
fn malformed_nonfinite_singular_and_overflowing_scopes_return_owned_errors() {
    for ops in [
        vec![DrawOp::EndTransform],
        vec![DrawOp::BeginTransform(Transform::IDENTITY)],
        vec![
            DrawOp::BeginTransform(Transform::IDENTITY),
            DrawOp::BeginTransform(Transform::IDENTITY),
            DrawOp::EndTransform,
            DrawOp::EndTransform,
        ],
        vec![
            DrawOp::BeginTransform(Transform {
                a: f32::NAN,
                ..Transform::IDENTITY
            }),
            DrawOp::EndTransform,
        ],
        vec![
            DrawOp::BeginTransform(Transform {
                a: 0.0,
                ..Transform::IDENTITY
            }),
            DrawOp::EndTransform,
        ],
        vec![
            DrawOp::BeginTransform(Transform {
                c: f32::MAX,
                ..Transform::IDENTITY
            }),
            DrawOp::EndTransform,
        ],
        vec![
            DrawOp::BeginTransform(Transform {
                a: f32::MAX,
                ..Transform::IDENTITY
            }),
            rectangle(),
            DrawOp::EndTransform,
        ],
    ] {
        assert!(matches!(
            render(&display(ops)),
            Err(PdfError::InvalidGeometry {
                page: 0,
                op: Some(_),
                ..
            })
        ));
    }
    let mut pages = display(vec![DrawOp::BeginTransform(Transform::IDENTITY)]);
    pages
        .pages
        .push(display(vec![DrawOp::EndTransform]).pages.remove(0));
    assert!(matches!(
        render(&pages),
        Err(PdfError::InvalidGeometry { page: 0, .. })
    ));
}
