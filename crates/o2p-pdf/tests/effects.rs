use o2p_core::{
    Color, DisplayList, DrawOp, ImageFormat, ImageOp, ImageResource, Page, Point, Rect, Shadow,
    ShadowGeometry, ShadowOp, Size,
};
use std::{io::Cursor, sync::Arc};

fn shadow(geometry: ShadowGeometry) -> ShadowOp {
    ShadowOp {
        geometry,
        fill: true,
        stroke_width: 0.0,
        shadow: Shadow {
            offset: Point { x: 4.0, y: 3.0 },
            blur_radius: 3.0,
            color: Color::rgb(12, 34, 56),
            opacity: 0.5,
        },
    }
}
fn display(op: ShadowOp) -> DisplayList {
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 100.0,
                height: 100.0,
            },
            ops: vec![DrawOp::Shadow(op)],
        }],
    }
}
fn bounds() -> Rect {
    Rect {
        origin: Point { x: 10.0, y: 10.0 },
        size: Size {
            width: 20.0,
            height: 20.0,
        },
    }
}
fn mask(pdf: &lopdf::Document) -> Vec<u8> {
    let image = pdf
        .objects
        .values()
        .find_map(|o| {
            let stream = o.as_stream().ok()?;
            stream.dict.get(b"SMask").ok()?.as_reference().ok()
        })
        .unwrap();
    let mut stream = pdf.get_object(image).unwrap().as_stream().unwrap().clone();
    stream.decompress().unwrap();
    stream.content
}
#[test]
fn shadow_color_and_alpha_are_local_pdf_image_resources() {
    let mut d = display(shadow(ShadowGeometry::RoundedRectangle {
        rect: bounds(),
        radius: 4.0,
    }));
    d.pages[0]
        .ops
        .push(DrawOp::RoundedRect(o2p_core::RoundedRectOp {
            rect: bounds(),
            radius: 4.0,
            color: Color::WHITE,
        }));
    let pdf = lopdf::Document::load_mem(&o2p_pdf::render(&d).unwrap()).unwrap();
    let alpha = mask(&pdf);
    assert_eq!(alpha.iter().max(), Some(&128));
    assert!(alpha.contains(&0));
    assert!(alpha.iter().any(|a| *a > 0 && *a < 128));
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap();
    let image = content
        .operations
        .iter()
        .position(|o| o.operator == "Do")
        .unwrap();
    let vector = content
        .operations
        .iter()
        .position(|o| o.operator == "c")
        .unwrap();
    assert!(image < vector);
    let images: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.dict
                .get(b"Subtype")
                .is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Image"))
        })
        .collect();
    assert_eq!(images.len(), 2);
    let stream = images
        .into_iter()
        .find(|s| s.dict.get(b"SMask").is_ok())
        .unwrap();
    let mut color = lopdf::Stream::clone(stream);
    color.decompress().unwrap();
    assert!(color.content.chunks_exact(3).all(|p| p == [12, 34, 56]));
}
#[test]
fn transparent_image_shadow_uses_image_alpha() {
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 0]));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let s = shadow(ShadowGeometry::Image(ImageOp {
        rect: bounds(),
        image: Arc::new(ImageResource {
            format: ImageFormat::Png,
            bytes: Arc::from(bytes.into_inner()),
        }),
    }));
    let pdf = lopdf::Document::load_mem(&o2p_pdf::render(&display(s)).unwrap()).unwrap();
    assert!(mask(&pdf).iter().all(|a| *a == 0));
}
#[test]
fn invalid_generic_shadow_parameters_fail_without_panicking() {
    for field in 0..6 {
        let mut s = shadow(ShadowGeometry::Rectangle(bounds()));
        match field {
            0 => s.shadow.opacity = f32::NAN,
            1 => s.shadow.opacity = 2.0,
            2 => s.shadow.blur_radius = -1.0,
            3 => s.shadow.offset.x = f32::INFINITY,
            4 => s.stroke_width = -1.0,
            _ => {
                s.geometry = ShadowGeometry::RoundedRectangle {
                    rect: bounds(),
                    radius: 11.0,
                }
            }
        };
        assert!(matches!(
            o2p_pdf::render(&display(s)),
            Err(o2p_pdf::PdfError::InvalidGeometry { .. })
        ));
    }
}
#[test]
fn huge_shadow_actual_pdf_dimensions_obey_limits() {
    let mut s = shadow(ShadowGeometry::Rectangle(bounds()));
    s.shadow.blur_radius = 1e20;
    let pdf = lopdf::Document::load_mem(&o2p_pdf::render(&display(s)).unwrap()).unwrap();
    for stream in pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.dict
                .get(b"Subtype")
                .is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Image"))
        })
    {
        let w = stream.dict.get(b"Width").unwrap().as_i64().unwrap();
        let h = stream.dict.get(b"Height").unwrap().as_i64().unwrap();
        assert!(w <= 2048 && h <= 2048 && w * h <= 1048576);
    }
}
