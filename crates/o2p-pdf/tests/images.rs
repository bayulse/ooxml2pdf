use image::ImageEncoder;
use o2p_core::{DisplayList, DrawOp, ImageFormat, ImageOp, ImageResource, Page, Point, Rect, Size};
use o2p_pdf::{PdfError, render};
use std::sync::Arc;

fn document(bytes: Vec<u8>, format: ImageFormat) -> DisplayList {
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 100.0,
                height: 100.0,
            },
            ops: vec![DrawOp::Image(ImageOp {
                rect: Rect {
                    origin: Point { x: 10.0, y: 20.0 },
                    size: Size {
                        width: 60.0,
                        height: 40.0,
                    },
                },
                image: Arc::new(ImageResource {
                    format,
                    bytes: bytes.into(),
                }),
            })],
        }],
    }
}

fn png(pixels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![];
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(pixels, 2, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

#[test]
fn rgba_alpha_is_encoded_as_a_soft_mask_and_pixels_remain_unpremultiplied() {
    let display = document(png(&[200, 100, 50, 128, 10, 20, 30, 0]), ImageFormat::Png);
    let bytes = render(&display).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let rgb = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .find(|s| s.dict.has(b"SMask"))
        .unwrap();
    assert_eq!(
        rgb.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
        b"DeviceRGB"
    );
    assert_eq!(
        rgb.decompressed_content().unwrap(),
        [200, 100, 50, 10, 20, 30]
    );
    let mask_id = rgb.dict.get(b"SMask").unwrap().as_reference().unwrap();
    let mask = pdf.get_object(mask_id).unwrap().as_stream().unwrap();
    assert_eq!(
        mask.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
        b"DeviceGray"
    );
    assert_eq!(mask.decompressed_content().unwrap(), [128, 0]);
    assert_eq!(bytes, render(&display).unwrap());
}

#[test]
fn grayscale_jpeg_decodes_and_shared_images_are_reused_across_pages() {
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100)
        .write_image(&[64, 64], 2, 1, image::ExtendedColorType::L8)
        .unwrap();
    let mut display = document(jpeg, ImageFormat::Jpeg);
    display.pages.push(display.pages[0].clone());
    let pdf = lopdf::Document::load_mem(&render(&display).unwrap()).unwrap();
    let images: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Image".as_slice())
        })
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].decompressed_content().unwrap(), [64; 6]);
    for id in pdf.get_pages().values() {
        let content = lopdf::content::Content::decode(&pdf.get_page_content(*id).unwrap()).unwrap();
        assert_eq!(
            content
                .operations
                .iter()
                .map(|op| op.operator.as_str())
                .collect::<Vec<_>>(),
            ["q", "cm", "Do", "Q"]
        );
        assert_eq!(
            content.operations[1]
                .operands
                .iter()
                .map(|o| o.as_float().unwrap())
                .collect::<Vec<_>>(),
            [60.0, 0.0, 0.0, 40.0, 10.0, 40.0]
        );
    }
}

#[test]
fn invalid_signature_corrupt_png_truncated_jpeg_and_invalid_geometry_are_errors() {
    for (bytes, format) in [
        (vec![], ImageFormat::Png),
        (vec![0xff, 0xd8, 0xff], ImageFormat::Jpeg),
        (png(&[0; 8]), ImageFormat::Jpeg),
        (b"\x89PNG\r\n\x1a\n".to_vec(), ImageFormat::Png),
    ] {
        assert!(matches!(
            render(&document(bytes, format)),
            Err(PdfError::InvalidImage { page: 0, op: 0, .. })
        ));
    }
    let mut bytes = png(&[0; 8]);
    bytes[29] ^= 1; // IHDR CRC
    assert!(matches!(
        render(&document(bytes, ImageFormat::Png)),
        Err(PdfError::InvalidImage { .. })
    ));
    for invalid in [f32::NAN, f32::INFINITY, -1.0] {
        let mut display = document(png(&[0; 8]), ImageFormat::Png);
        let DrawOp::Image(image) = &mut display.pages[0].ops[0] else {
            unreachable!()
        };
        image.rect.size.width = invalid;
        assert!(matches!(
            render(&display),
            Err(PdfError::InvalidGeometry {
                page: 0,
                op: Some(0),
                ..
            })
        ));
    }
}

#[test]
fn image_dimensions_and_pixel_count_are_bounded_before_pixel_allocation() {
    for (width, height) in [(16385u32, 1u32), (8192, 8192)] {
        let mut bytes = png(&[0; 8]);
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        let crc = crc32fast::hash(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        let error = render(&document(bytes, ImageFormat::Png)).unwrap_err();
        assert!(matches!(error, PdfError::InvalidImage { .. }));
        assert!(
            error.to_string().contains("limit") || error.to_string().contains("max_image_pixels"),
            "{error}"
        );
    }
}
