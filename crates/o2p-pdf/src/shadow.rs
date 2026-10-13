//! Bounded local silhouette masks. Original element paint remains vector.
use o2p_core::{
    DisplayList, DrawOp, ImageFormat, ImageOp, ImageResource, Point, Rect, ShadowGeometry,
    ShadowOp, Size,
};
use std::{io::Cursor, sync::Arc};

const MAX_DIMENSION: usize = 2048;
const MAX_PIXELS: usize = 1_048_576;
const PIXELS_PER_POINT: f64 = 2.0;

fn dimensions(width: f64, height: f64) -> (usize, usize, f64) {
    let scale = PIXELS_PER_POINT
        .min(MAX_DIMENSION as f64 / width)
        .min(MAX_DIMENSION as f64 / height)
        .min(((MAX_PIXELS as f64) / (width * height)).sqrt());
    // Floor makes rounding obey both hard bounds, including very thin shapes.
    let w = (width * scale).floor().max(1.0) as usize;
    let h = (height * scale).floor().max(1.0) as usize;
    (w.min(MAX_DIMENSION), h.min(MAX_DIMENSION), scale)
}

pub(super) fn validate(op: &ShadowOp) -> Result<(), &'static str> {
    let s = op.shadow;
    if !s.offset.x.is_finite()
        || !s.offset.y.is_finite()
        || !s.blur_radius.is_finite()
        || s.blur_radius < 0.0
        || !s.opacity.is_finite()
        || !(0.0..=1.0).contains(&s.opacity)
        || !op.stroke_width.is_finite()
        || op.stroke_width < 0.0
    {
        return Err("shadow parameters");
    }
    if let ShadowGeometry::RoundedRectangle { rect, radius } = &op.geometry {
        if !radius.is_finite()
            || *radius < 0.0
            || *radius > rect.size.width.min(rect.size.height) / 2.0
        {
            return Err("shadow corner radius");
        }
    }
    let rect = extent(op);
    if !rect.origin.x.is_finite()
        || !rect.origin.y.is_finite()
        || !rect.size.width.is_finite()
        || !rect.size.height.is_finite()
    {
        return Err("shadow extent");
    }
    Ok(())
}
fn extent(op: &ShadowOp) -> Rect {
    let r = op.geometry.bounds();
    let pad = f64::from(op.shadow.blur_radius) * 1.5 + f64::from(op.stroke_width) / 2.0 + 1.0;
    Rect {
        origin: Point {
            x: (f64::from(r.origin.x) + f64::from(op.shadow.offset.x) - pad) as f32,
            y: (f64::from(r.origin.y) + f64::from(op.shadow.offset.y) - pad) as f32,
        },
        size: Size {
            width: (f64::from(r.size.width) + 2.0 * pad) as f32,
            height: (f64::from(r.size.height) + 2.0 * pad) as f32,
        },
    }
}
fn distance(op: &ShadowOp, x: f64, y: f64) -> f64 {
    let r = op.geometry.bounds();
    let hx = f64::from(r.size.width) / 2.0;
    let hy = f64::from(r.size.height) / 2.0;
    let x = (x - hx).abs();
    let y = (y - hy).abs();
    if matches!(op.geometry, ShadowGeometry::Ellipse(_)) {
        let k0 = ((x / hx).powi(2) + (y / hy).powi(2)).sqrt();
        let k1 = ((x / (hx * hx)).powi(2) + (y / (hy * hy)).powi(2)).sqrt();
        return if k1 == 0.0 {
            -hx.min(hy)
        } else {
            k0 * (k0 - 1.0) / k1
        };
    }
    let radius = match op.geometry {
        ShadowGeometry::RoundedRectangle { radius, .. } => f64::from(radius),
        _ => 0.0,
    };
    let qx = x - hx + radius;
    let qy = y - hy + radius;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
}

fn rasterize(op: &ShadowOp) -> Result<ImageOp, String> {
    validate(op).map_err(str::to_owned)?;
    let rect = extent(op);
    let (w, h, scale) = dimensions(f64::from(rect.size.width), f64::from(rect.size.height));
    let r = op.geometry.bounds();
    let dx = f64::from(r.origin.x) + f64::from(op.shadow.offset.x) - f64::from(rect.origin.x);
    let dy = f64::from(r.origin.y) + f64::from(op.shadow.offset.y) - f64::from(rect.origin.y);
    let sx = f64::from(rect.size.width) / w as f64;
    let sy = f64::from(rect.size.height) / h as f64;
    let image = if let ShadowGeometry::Image(i) = &op.geometry {
        Some(crate::raster::decode(&i.image)?.into_rgba8())
    } else {
        None
    };
    let mut mask = vec![0.0_f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut coverage = 0.0;
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let px = (x as f64 + ox) * sx - dx;
                let py = (y as f64 + oy) * sy - dy;
                if let Some(image) = &image {
                    if px >= 0.0
                        && py >= 0.0
                        && px < f64::from(r.size.width)
                        && py < f64::from(r.size.height)
                    {
                        let ix = (px / f64::from(r.size.width) * f64::from(image.width())) as u32;
                        let iy = (py / f64::from(r.size.height) * f64::from(image.height())) as u32;
                        coverage += f32::from(
                            image.get_pixel(ix.min(image.width() - 1), iy.min(image.height() - 1))
                                [3],
                        ) / 255.0;
                    }
                } else {
                    let d = distance(op, px, py);
                    if (op.fill && d <= 0.0)
                        || (op.stroke_width > 0.0 && d.abs() <= f64::from(op.stroke_width) / 2.0)
                    {
                        coverage += 1.0;
                    }
                }
            }
            mask[y * w + x] = coverage / 4.0;
        }
    }
    drop(image);
    // Three linear-time box passes approximate a Gaussian with sigma=blur_radius/2.
    // Fractional radii blend adjacent box kernels, preserving small shadow blur.
    let sigma = f64::from(op.shadow.blur_radius) * scale / 2.0;
    let box_radius = ((1.0 + 4.0 * sigma * sigma).sqrt() - 1.0) / 2.0;
    if box_radius > 0.0 {
        let radius = box_radius.floor().min(MAX_DIMENSION as f64) as usize;
        let fraction = (box_radius - radius as f64).clamp(0.0, 1.0) as f32;
        let mut scratch = vec![0.0_f32; w * h];
        for _ in 0..3 {
            blur_axis(&mask, &mut scratch, w, h, radius, fraction, true);
            blur_axis(&scratch, &mut mask, w, h, radius, fraction, false);
        }
    }
    let color = op.shadow.color;
    let mut rgba = vec![0_u8; w * h * 4];
    for (pixel, alpha) in rgba.chunks_exact_mut(4).zip(mask) {
        pixel.copy_from_slice(&[
            color.red,
            color.green,
            color.blue,
            (alpha * op.shadow.opacity * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8,
        ]);
    }
    let mut encoded = Cursor::new(Vec::new());
    image::RgbaImage::from_raw(w as u32, h as u32, rgba)
        .ok_or("invalid shadow image size")?
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(ImageOp {
        rect,
        image: Arc::new(ImageResource {
            format: ImageFormat::Png,
            bytes: Arc::from(encoded.into_inner()),
        }),
    })
}
fn blur_axis(
    src: &[f32],
    dst: &mut [f32],
    w: usize,
    h: usize,
    radius: usize,
    fraction: f32,
    horizontal: bool,
) {
    let (lines, len) = if horizontal { (h, w) } else { (w, h) };
    for line in 0..lines {
        let index = |i: usize| {
            if horizontal {
                line * w + i
            } else {
                i * w + line
            }
        };
        let mut sum: f32 = (0..=radius.min(len - 1)).map(|i| src[index(i)]).sum();
        for i in 0..len {
            let mut wider = sum;
            if i > radius {
                wider += src[index(i - radius - 1)];
            }
            if i + radius + 1 < len {
                wider += src[index(i + radius + 1)];
            }
            dst[index(i)] = sum / (2 * radius + 1) as f32 * (1.0 - fraction)
                + wider / (2 * radius + 3) as f32 * fraction;
            if i >= radius {
                sum -= src[index(i - radius)];
            }
            if i + radius + 1 < len {
                sum += src[index(i + radius + 1)];
            }
        }
    }
}
pub(super) fn expand(display: &DisplayList) -> Result<DisplayList, super::PdfError> {
    let mut expanded = display.clone();
    for (page, p) in expanded.pages.iter_mut().enumerate() {
        for (op_index, op) in p.ops.iter_mut().enumerate() {
            if let DrawOp::Shadow(shadow) = op {
                *op = DrawOp::Image(rasterize(shadow).map_err(|message| {
                    super::PdfError::InvalidImage {
                        page,
                        op: op_index,
                        message,
                    }
                })?);
            }
        }
    }
    Ok(expanded)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocations_are_bounded() {
        for (w, h) in [
            (100.0, 50.0),
            (1e30, 1e30),
            (1e30, 1.0),
            (1.0, 1e30),
            (4096.0, 4096.0),
        ] {
            let (w, h, _) = dimensions(w, h);
            assert!(w <= MAX_DIMENSION && h <= MAX_DIMENSION && w * h <= MAX_PIXELS);
        }
    }
    #[test]
    fn blur_has_transparent_boundaries() {
        let mut dst = vec![0.0; 3];
        blur_axis(&[0.0, 1.0, 0.0], &mut dst, 3, 1, 1, 0.0, true);
        assert_eq!(dst, vec![1.0 / 3.0; 3]);
    }
}
