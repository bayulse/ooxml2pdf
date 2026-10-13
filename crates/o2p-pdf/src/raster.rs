//! Raster decoding is private to this backend; no Office or package semantics.
use std::io::{Cursor, Write};

use flate2::{Compression, write::ZlibEncoder};
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use o2p_core::{ImageFormat, ImageResource};
use pdf_writer::{Filter, Finish, Pdf, Ref};

const MAX_PIXELS: u64 = 16_777_216;
const MAX_DIMENSION: u32 = 16_384;
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;

pub(super) fn decode(resource: &ImageResource) -> Result<DynamicImage, String> {
    if ImageFormat::detect(&resource.bytes) != Some(resource.format) {
        return Err("image format does not match signature".into());
    }
    if resource.format == ImageFormat::Png {
        // Animated PNG is outside the raster contract; do not silently choose a frame.
        let mut rest = &resource.bytes[8..];
        while rest.len() >= 12 {
            let length =
                u32::from_be_bytes(rest[..4].try_into().map_err(|_| "invalid PNG chunk")?) as usize;
            if &rest[4..8] == b"acTL" {
                return Err("animated PNG is unsupported".into());
            }
            let size = length.checked_add(12).ok_or("invalid PNG chunk length")?;
            rest = rest.get(size..).ok_or("truncated PNG chunk")?;
        }
        if !rest.is_empty() {
            return Err("trailing PNG bytes".into());
        }
    }
    let format = match resource.format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
    };
    let mut reader = ImageReader::with_format(Cursor::new(resource.bytes.as_ref()), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("image exceeds max_image_pixels (16777216) or has zero dimensions".into());
    }
    if decoder.total_bytes() > MAX_DECODE_BYTES {
        return Err("image exceeds max_decode_bytes".into());
    }
    DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())
}
pub(super) fn write(pdf: &mut Pdf, reference: Ref, resource: &ImageResource) -> Result<(), String> {
    let decoded = decode(resource)?;
    let (width, height) = (decoded.width(), decoded.height());
    let (rgb, alpha) = if decoded.color().has_alpha() {
        let rgba = decoded.into_rgba8();
        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        let mut alpha = Vec::with_capacity(width as usize * height as usize);
        let mut opaque = true;
        for pixel in rgba.as_raw().chunks_exact(4) {
            rgb.extend_from_slice(&pixel[..3]);
            alpha.push(pixel[3]);
            opaque &= pixel[3] == 255;
        }
        (rgb, if opaque { None } else { Some(alpha) })
    } else {
        (decoded.into_rgb8().into_raw(), None)
    };
    let compressed = compress(&rgb)?;
    let mut object = pdf.image_xobject(reference, &compressed);
    object
        .width(width as i32)
        .height(height as i32)
        .bits_per_component(8)
        .filter(Filter::FlateDecode);
    object.color_space().device_rgb();
    if alpha.is_some() {
        object.s_mask(Ref::new(reference.get() + 1));
    }
    object.finish();
    if let Some(alpha) = alpha {
        let compressed = compress(&alpha)?;
        let mut mask = pdf.image_xobject(Ref::new(reference.get() + 1), &compressed);
        mask.width(width as i32)
            .height(height as i32)
            .bits_per_component(8)
            .filter(Filter::FlateDecode);
        mask.color_space().device_gray();
        mask.finish();
    }
    Ok(())
}

fn compress(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    encoder.finish().map_err(|error| error.to_string())
}
