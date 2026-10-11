//! Deterministic PDF rendering of a format-independent display list.
//!
//! The public API contains only standard-library and `o2p-core` types.
//! Milestone 0 maps the logical sans-serif face to PDF's standard Helvetica and
//! accepts printable ASCII only. No system fonts or external processes are used.

mod raster;

use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use o2p_core::{Color, DisplayList, DrawOp, Font, Point};
use pdf_writer::{Content, Finish, Name, Pdf, Ref, Str};

/// A rendering failure. Page and operation indices are zero-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    InvalidImage {
        page: usize,
        op: usize,
        message: String,
    },
    EmptyDocument,
    TooManyPages,
    InvalidGeometry {
        page: usize,
        op: Option<usize>,
        field: &'static str,
    },
    UnsupportedText {
        page: usize,
        op: usize,
        character: char,
    },
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImage { page, op, message } => {
                write!(f, "invalid image on page {page}, operation {op}: {message}")
            }
            Self::EmptyDocument => write!(f, "a PDF must contain at least one page"),
            Self::TooManyPages => write!(f, "document exceeds the PDF object limit"),
            Self::InvalidGeometry { page, op, field } => {
                write!(f, "invalid {field} on page {page}")?;
                if let Some(op) = op {
                    write!(f, ", operation {op}")?;
                }
                Ok(())
            }
            Self::UnsupportedText {
                page,
                op,
                character,
            } => write!(
                f,
                "unsupported character {character:?} on page {page}, operation {op}; only printable ASCII is supported"
            ),
        }
    }
}

impl Error for PdfError {}

/// Render all pages to PDF 1.7 bytes, with no timestamps or random identifiers.
///
/// The whole display list is validated before generating output. Geometry must
/// be finite; pages, font sizes and line widths must be positive; rectangle
/// dimensions may be zero. Off-page content is permitted and clipped by the page.
/// The same input produces identical bytes with the same backend version.
///
/// ```
/// use o2p_core::{DisplayList, Page, Size};
/// let document = DisplayList {
///     pages: vec![Page {
///         size: Size { width: 595.0, height: 842.0 },
///         ops: vec![],
///     }],
/// };
/// let bytes = o2p_pdf::render(&document)?;
/// assert!(bytes.starts_with(b"%PDF-1.7"));
/// # Ok::<(), o2p_pdf::PdfError>(())
/// ```
pub fn render(display_list: &DisplayList) -> Result<Vec<u8>, PdfError> {
    validate(display_list)?;

    let catalog = Ref::new(1);
    let pages = Ref::new(2);
    let font = Ref::new(3);
    let font_name = Name(b"F1");
    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(pages);
    // Validation ensures these object numbers and counts fit into i32.
    let page_ids: Vec<_> = (0..display_list.pages.len())
        .map(|index| Ref::new(4 + index as i32 * 2))
        .collect();
    pdf.pages(pages)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    pdf.type1_font(font).base_font(Name(b"Helvetica"));

    // Pointer identity preserves shared resources without hashing/copying large bytes.
    // Each decoder's pixel buffers are released immediately after its XObject is written.
    let mut images = BTreeMap::new();
    let mut next_object = 4 + display_list.pages.len() as i32 * 2;
    for (page_index, page) in display_list.pages.iter().enumerate() {
        for (op_index, op) in page.ops.iter().enumerate() {
            if let DrawOp::Image(image) = op {
                let key = Arc::as_ptr(&image.image) as usize;
                if let std::collections::btree_map::Entry::Vacant(entry) = images.entry(key) {
                    let reference = Ref::new(next_object);
                    next_object = next_object.checked_add(2).ok_or(PdfError::TooManyPages)?;
                    let name = format!("I{}", reference.get());
                    raster::write(&mut pdf, reference, &image.image).map_err(|message| {
                        PdfError::InvalidImage {
                            page: page_index,
                            op: op_index,
                            message,
                        }
                    })?;
                    entry.insert((name, reference));
                }
            }
        }
    }

    for (page, page_id) in display_list.pages.iter().zip(page_ids) {
        let stream_id = Ref::new(page_id.get() + 1);
        let mut pdf_page = pdf.page(page_id);
        pdf_page.parent(pages);
        pdf_page.media_box(pdf_writer::Rect::new(
            0.0,
            0.0,
            page.size.width,
            page.size.height,
        ));
        pdf_page.contents(stream_id);
        {
            let mut resources = pdf_page.resources();
            resources.fonts().pair(font_name, font);
            if page.ops.iter().any(|op| matches!(op, DrawOp::Image(_))) {
                let mut xobjects = resources.x_objects();
                let mut used = std::collections::BTreeSet::new();
                for op in &page.ops {
                    if let DrawOp::Image(image) = op {
                        let (name, reference) = &images[&(Arc::as_ptr(&image.image) as usize)];
                        if used.insert(reference.get()) {
                            xobjects.pair(Name(name.as_bytes()), *reference);
                        }
                    }
                }
            }
        }
        pdf_page.finish();

        let mut content = Content::new();
        for op in &page.ops {
            // Every operation is isolated from the previous graphics state.
            content.save_state();
            match op {
                DrawOp::Image(image) => {
                    let (name, _) = &images[&(Arc::as_ptr(&image.image) as usize)];
                    content.transform([
                        image.rect.size.width,
                        0.0,
                        0.0,
                        image.rect.size.height,
                        image.rect.origin.x,
                        page.size.height - image.rect.origin.y - image.rect.size.height,
                    ]);
                    content.x_object(Name(name.as_bytes()));
                }
                DrawOp::Text(text) => {
                    let Font::SansSerif = text.font;
                    fill_color(&mut content, text.color);
                    content.begin_text();
                    content.set_font(font_name, text.font_size);
                    content.set_text_matrix([
                        1.0,
                        0.0,
                        0.0,
                        1.0,
                        text.origin.x,
                        page.size.height - text.origin.y,
                    ]);
                    content.show(Str(text.text.as_bytes()));
                    content.end_text();
                }
                DrawOp::Rect(rect) => {
                    fill_color(&mut content, rect.color);
                    content.rect(
                        rect.rect.origin.x,
                        page.size.height - rect.rect.origin.y - rect.rect.size.height,
                        rect.rect.size.width,
                        rect.rect.size.height,
                    );
                    content.fill_nonzero();
                }
                DrawOp::Line(line) => {
                    let [r, g, b] = rgb(line.color);
                    content.set_stroke_rgb(r, g, b);
                    content.set_line_width(line.width);
                    content.move_to(line.start.x, page.size.height - line.start.y);
                    content.line_to(line.end.x, page.size.height - line.end.y);
                    content.stroke();
                }
                DrawOp::Ellipse(ellipse) => {
                    fill_color(&mut content, ellipse.color);
                    ellipse_path(&mut content, ellipse.bounds, page.size.height);
                    content.fill_nonzero();
                }
                DrawOp::StrokeEllipse(ellipse) => {
                    let [r, g, b] = rgb(ellipse.color);
                    content.set_stroke_rgb(r, g, b);
                    content.set_line_width(ellipse.width);
                    ellipse_path(&mut content, ellipse.bounds, page.size.height);
                    content.stroke();
                }
                DrawOp::StrokeRect(rect) => {
                    let [r, g, b] = rgb(rect.color);
                    content.set_stroke_rgb(r, g, b);
                    content.set_line_width(rect.width);
                    content.rect(
                        rect.rect.origin.x,
                        page.size.height - rect.rect.origin.y - rect.rect.size.height,
                        rect.rect.size.width,
                        rect.rect.size.height,
                    );
                    content.stroke();
                }
            }
            content.restore_state();
        }
        pdf.stream(stream_id, &content.finish());
    }
    Ok(pdf.finish())
}

fn rgb(color: Color) -> [f32; 3] {
    [color.red, color.green, color.blue].map(|channel| f32::from(channel) / 255.0)
}

/// Four cubic quarter-arcs; maximum radial error is about 0.028% of radius.
fn ellipse_path(content: &mut Content, bounds: o2p_core::Rect, page_height: f32) {
    // Calculate intermediates in f64 to avoid overflow for valid f32 bounds.
    let x = f64::from(bounds.origin.x);
    let y = f64::from(bounds.origin.y);
    let rx = f64::from(bounds.size.width) / 2.0;
    let ry = f64::from(bounds.size.height) / 2.0;
    let cx = x + rx;
    let cy = y + ry;
    let k = 0.552_284_749_830_793_6;
    let px = |v: f64| v as f32;
    let py = |v: f64| (f64::from(page_height) - v) as f32;
    content.move_to(px(x + 2.0 * rx), py(cy));
    content.cubic_to(
        px(x + 2.0 * rx),
        py(cy + k * ry),
        px(cx + k * rx),
        py(y + 2.0 * ry),
        px(cx),
        py(y + 2.0 * ry),
    );
    content.cubic_to(
        px(cx - k * rx),
        py(y + 2.0 * ry),
        px(x),
        py(cy + k * ry),
        px(x),
        py(cy),
    );
    content.cubic_to(
        px(x),
        py(cy - k * ry),
        px(cx - k * rx),
        py(y),
        px(cx),
        py(y),
    );
    content.cubic_to(
        px(cx + k * rx),
        py(y),
        px(x + 2.0 * rx),
        py(cy - k * ry),
        px(x + 2.0 * rx),
        py(cy),
    );
    content.close_path();
}

fn fill_color(content: &mut Content, color: Color) {
    let [r, g, b] = rgb(color);
    content.set_fill_rgb(r, g, b);
}

fn validate(display_list: &DisplayList) -> Result<(), PdfError> {
    if display_list.pages.is_empty() {
        return Err(PdfError::EmptyDocument);
    }
    if display_list.pages.len() > ((i32::MAX - 4) / 2) as usize {
        return Err(PdfError::TooManyPages);
    }
    for (page_index, page) in display_list.pages.iter().enumerate() {
        let check = |valid, op, field| {
            if valid {
                Ok(())
            } else {
                Err(PdfError::InvalidGeometry {
                    page: page_index,
                    op,
                    field,
                })
            }
        };
        check(
            positive(page.size.width) && positive(page.size.height),
            None,
            "page size",
        )?;
        for (op_index, op) in page.ops.iter().enumerate() {
            let index = Some(op_index);
            match op {
                DrawOp::Image(image) => {
                    validate_rect(image.rect, page.size.height).map_err(|field| {
                        PdfError::InvalidGeometry {
                            page: page_index,
                            op: index,
                            field,
                        }
                    })?;
                }
                DrawOp::Text(text) => {
                    check(finite_point(text.origin), index, "text origin")?;
                    check(
                        (page.size.height - text.origin.y).is_finite(),
                        index,
                        "text baseline",
                    )?;
                    check(positive(text.font_size), index, "font size")?;
                    if let Some(character) = text.text.chars().find(|c| !(' '..='~').contains(c)) {
                        return Err(PdfError::UnsupportedText {
                            page: page_index,
                            op: op_index,
                            character,
                        });
                    }
                }
                DrawOp::StrokeRect(rect) => {
                    check(positive(rect.width), index, "rectangle outline width")?;
                    validate_rect(rect.rect, page.size.height).map_err(|field| {
                        PdfError::InvalidGeometry {
                            page: page_index,
                            op: index,
                            field,
                        }
                    })?;
                }
                DrawOp::Rect(rect) => {
                    validate_rect(rect.rect, page.size.height).map_err(|field| {
                        PdfError::InvalidGeometry {
                            page: page_index,
                            op: index,
                            field,
                        }
                    })?;
                }
                DrawOp::Ellipse(ellipse) => {
                    validate_ellipse(ellipse.bounds, page.size.height).map_err(|field| {
                        PdfError::InvalidGeometry {
                            page: page_index,
                            op: index,
                            field,
                        }
                    })?;
                }
                DrawOp::StrokeEllipse(ellipse) => {
                    check(positive(ellipse.width), index, "ellipse outline width")?;
                    validate_ellipse(ellipse.bounds, page.size.height).map_err(|field| {
                        PdfError::InvalidGeometry {
                            page: page_index,
                            op: index,
                            field,
                        }
                    })?;
                }
                DrawOp::Line(line) => {
                    check(
                        finite_point(line.start) && finite_point(line.end),
                        index,
                        "line endpoints",
                    )?;
                    check(
                        (page.size.height - line.start.y).is_finite()
                            && (page.size.height - line.end.y).is_finite(),
                        index,
                        "line position",
                    )?;
                    check(positive(line.width), index, "line width")?;
                }
            }
        }
    }
    Ok(())
}

fn validate_ellipse(bounds: o2p_core::Rect, page_height: f32) -> Result<(), &'static str> {
    validate_rect(bounds, page_height)?;
    // Unlike a rectangle operator, the curve also emits the top boundary.
    if !(page_height - bounds.origin.y).is_finite() {
        return Err("ellipse position");
    }
    Ok(())
}

fn validate_rect(rect: o2p_core::Rect, page_height: f32) -> Result<(), &'static str> {
    if !finite_point(rect.origin) {
        return Err("rectangle origin");
    }
    if !nonnegative(rect.size.width) || !nonnegative(rect.size.height) {
        return Err("rectangle size");
    }
    if !(page_height - rect.origin.y - rect.size.height).is_finite()
        || !(rect.origin.x + rect.size.width).is_finite()
        || !(rect.origin.y + rect.size.height).is_finite()
    {
        return Err("rectangle position");
    }
    Ok(())
}

fn finite_point(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}
fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}
fn nonnegative(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_channels_are_normalized() {
        assert_eq!(rgb(Color::BLACK), [0.0; 3]);
        assert_eq!(rgb(Color::WHITE), [1.0; 3]);
        assert_eq!(rgb(Color::rgb(255, 0, 255)), [1.0, 0.0, 1.0]);
    }

    #[test]
    fn empty_document_is_an_error() {
        assert_eq!(
            render(&DisplayList::default()),
            Err(PdfError::EmptyDocument)
        );
    }
}
