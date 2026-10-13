//! Compatibility layout using the fixed standard-font advances, without wrapping.
use crate::{Alignment, Paragraph};
use o2p_core::{Font, Point, Rect, TextOp, VerticalAlignment};

/// Legacy fixed-font layout. Family availability/Unicode limitations belong to
/// the reporting PDF path; configured-font callers should use `layout_text_box`.
pub fn layout_unwrapped(
    paragraphs: &[Paragraph],
    bounds: Rect,
    anchor: VerticalAlignment,
) -> (Vec<TextOp>, bool) {
    let mut ops = Vec::new();
    let mut top = f64::from(bounds.origin.y);
    let bottom = top + f64::from(bounds.size.height);
    let mut overflow = false;
    for paragraph in paragraphs {
        let mut lines = vec![Vec::new()];
        for run in &paragraph.runs {
            for (index, segment) in run.text.split('\n').enumerate() {
                if index != 0 {
                    lines.push(Vec::new());
                }
                if let Some(line) = lines.last_mut() {
                    line.push((segment, run));
                }
            }
        }
        let last = lines.len() - 1;
        let mut last_size = f64::from(paragraph.empty_font_size);
        for (index, line) in lines.into_iter().enumerate() {
            let size = if index == last && line.iter().all(|(text, _)| text.is_empty()) {
                f64::from(paragraph.empty_font_size)
            } else {
                line.iter()
                    .map(|(_, run)| f64::from(run.font_size))
                    .reduce(f64::max)
                    .unwrap_or(f64::from(paragraph.empty_font_size))
            };
            if index == 0 {
                top += paragraph.format.space_before.points(size);
            }
            last_size = size;
            let offset = f64::from(paragraph.format.margin_left)
                + if index == 0 && paragraph.bullet.is_none() {
                    f64::from(paragraph.format.first_line_indent)
                } else {
                    0.0
                };
            let available =
                (f64::from(bounds.size.width) - offset - f64::from(paragraph.format.margin_right))
                    .max(0.0);
            let width: f64 = line
                .iter()
                .map(|(text, run)| {
                    face(run.font.weight, run.font.italic).text_width(text, run.font_size)
                })
                .sum();
            let mut x = f64::from(bounds.origin.x)
                + offset
                + match paragraph.alignment {
                    Alignment::Left => 0.0,
                    Alignment::Center => (available - width) / 2.0,
                    Alignment::Right => available - width,
                };
            let baseline = top + size;
            overflow |= width > available || top + size * 1.2 > bottom;
            if index == 0 && paragraph.runs.iter().any(|r| !r.text.is_empty()) {
                if let Some(bullet) = &paragraph.bullet {
                    ops.push(TextOp {
                        origin: Point {
                            x: (x + f64::from(paragraph.format.first_line_indent)) as f32,
                            y: baseline as f32,
                        },
                        text: bullet.glyph.clone(),
                        font: face(bullet.font.weight, bullet.font.italic),
                        font_size: bullet.font_size,
                        color: bullet.color,
                    });
                }
            }
            for (text, run) in line {
                let font = face(run.font.weight, run.font.italic);
                if !text.is_empty() {
                    ops.push(TextOp {
                        origin: Point {
                            x: x as f32,
                            y: baseline as f32,
                        },
                        text: text.into(),
                        font,
                        font_size: run.font_size,
                        color: run.color,
                    });
                }
                x += font.text_width(text, run.font_size);
            }
            top += size * 1.2;
        }
        top += paragraph.format.space_after.points(last_size);
    }
    let height = top - f64::from(bounds.origin.y);
    let offset = match anchor {
        VerticalAlignment::Top => 0.0,
        VerticalAlignment::Center => (f64::from(bounds.size.height) - height) / 2.0,
        VerticalAlignment::Bottom => f64::from(bounds.size.height) - height,
    };
    for op in &mut ops {
        op.origin.y = (f64::from(op.origin.y) + offset) as f32;
    }
    overflow |= top > bottom;
    (ops, overflow)
}
fn face(weight: u16, italic: bool) -> Font {
    Font::sans_serif(weight >= 700, italic)
}
