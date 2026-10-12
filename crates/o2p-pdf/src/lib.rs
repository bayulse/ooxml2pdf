//! Deterministic PDF rendering of a format-independent display list.
//!
//! The public API contains only standard-library and `o2p-core` types.
//! Legacy text uses standard Helvetica/WinAnsi; shaped text embeds configured
//! TrueType fonts with Unicode mappings. Missing glyphs are reported explicitly.
//! No system fonts, shaping, or processes are used by this serialization crate.

mod embedded;
mod raster;
mod text;

use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use o2p_core::{Color, DisplayList, DrawOp, Font, Point, Transform};
use pdf_writer::{Content, Finish, Name, Pdf, Ref, Str};

/// A rendering failure. Page and operation indices are zero-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    InvalidFont {
        page: usize,
        op: usize,
        message: String,
    },
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
            Self::InvalidFont { page, op, message } => write!(
                f,
                "invalid font/text on page {page}, operation {op}: {message}"
            ),
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
                "unsupported character {character:?} on page {page}, operation {op}; the fixed fallback cannot encode this character"
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
/// Strict rendering preserves the original error policy for missing glyphs.
pub fn render(display_list: &DisplayList) -> Result<Vec<u8>, PdfError> {
    Ok(render_internal(display_list, false)?.bytes)
}

/// A deterministic missing-glyph warning from the selected text adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfWarning {
    pub page: usize,
    pub op: usize,
    /// Distinct missing characters in first occurrence order; see `kind`.
    pub missing_glyphs: Vec<char>,
    pub kind: PdfWarningKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfWarningKind {
    LegacyMissingGlyph,
    EmbeddedMissingGlyph,
}

impl fmt::Display for PdfWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind == PdfWarningKind::EmbeddedMissingGlyph {
            return write!(
                f,
                "page {}, operation {}: embedded font has no glyphs {:?}; rendered as .notdef, Unicode source preserved",
                self.page, self.op, self.missing_glyphs
            );
        }
        write!(
            f,
            "page {}, operation {}: fixed sans-serif fallback has no glyphs for {:?}; rendered as '?'",
            self.page, self.op, self.missing_glyphs
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfReport {
    pub bytes: Vec<u8>,
    pub warnings: Vec<PdfWarning>,
}

/// Render with warnings: legacy text uses `?`, embedded text uses `.notdef`.
/// Control characters remain errors; callers must lay out line breaks separately.
pub fn render_with_report(display_list: &DisplayList) -> Result<PdfReport, PdfError> {
    render_internal(display_list, true)
}

fn render_internal(display_list: &DisplayList, fallback: bool) -> Result<PdfReport, PdfError> {
    let warnings = validate(display_list, fallback)?;

    let catalog = Ref::new(1);
    let pages = Ref::new(2);
    let font = Ref::new(3);
    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(pages);
    // Validation ensures these object numbers and counts fit into i32.
    let page_ids: Vec<_> = (0..display_list.pages.len())
        .map(|index| Ref::new(4 + index as i32 * 2))
        .collect();
    pdf.pages(pages)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    let extra_start = 4 + display_list.pages.len() as i32 * 2;
    let fonts = [
        font,
        Ref::new(extra_start),
        Ref::new(extra_start + 1),
        Ref::new(extra_start + 2),
    ];
    for face in [
        Font::SansSerif,
        Font::SansSerifBold,
        Font::SansSerifItalic,
        Font::SansSerifBoldItalic,
    ] {
        let (_, base, index) = text::face(face);
        pdf.type1_font(fonts[index])
            .base_font(Name(base))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }

    // Pointer identity preserves shared resources without hashing/copying large bytes.
    // Each decoder's pixel buffers are released immediately after its XObject is written.
    let mut images = BTreeMap::new();
    let mut next_object = extra_start + 3;
    let embedded_fonts = embedded::prepare(display_list, &mut next_object)?;
    embedded::write(&mut pdf, &embedded_fonts);
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
            {
                let mut resource_fonts = resources.fonts();
                for font in embedded::ordered(&embedded_fonts) {
                    resource_fonts.pair(Name(font.name.as_bytes()), font.reference);
                }
                for face in [
                    Font::SansSerif,
                    Font::SansSerifBold,
                    Font::SansSerifItalic,
                    Font::SansSerifBoldItalic,
                ] {
                    let (name, _, index) = text::face(face);
                    resource_fonts.pair(name, fonts[index]);
                }
            }
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
            match op {
                DrawOp::BeginTransform(transform) => {
                    content.save_state();
                    content.transform(pdf_transform(*transform, page.size.height));
                    continue;
                }
                DrawOp::EndTransform => {
                    content.restore_state();
                    continue;
                }
                _ => {}
            }
            // Every operation is isolated from the previous graphics state.
            content.save_state();
            match op {
                DrawOp::BeginTransform(_) | DrawOp::EndTransform => unreachable!("handled above"),
                DrawOp::GlyphText(text) => {
                    embedded::draw(&mut content, &embedded_fonts, text, page.size.height)
                }
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
                    let (font_name, _, _) = text::face(text.font);
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
                    let encoded: Vec<_> = text
                        .text
                        .chars()
                        .map(|c| text::encode(c).unwrap_or(b'?'))
                        .collect();
                    content.show(Str(&encoded));
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
    Ok(PdfReport {
        bytes: pdf.finish(),
        warnings,
    })
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

fn validate(display_list: &DisplayList, fallback: bool) -> Result<Vec<PdfWarning>, PdfError> {
    let mut warnings = vec![];
    if display_list.pages.is_empty() {
        return Err(PdfError::EmptyDocument);
    }
    if display_list.pages.len() > ((i32::MAX - 7) / 2) as usize {
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
        let mut active_transform = None;
        for (op_index, op) in page.ops.iter().enumerate() {
            let index = Some(op_index);
            if let Some(transform) = active_transform {
                check(
                    transformed_geometry_is_finite(op, transform, page.size.height),
                    index,
                    "transformed position",
                )?;
            }
            match op {
                DrawOp::BeginTransform(transform) => {
                    check(active_transform.is_none(), index, "nested transform scope")?;
                    let matrix = pdf_transform(*transform, page.size.height);
                    let determinant = f64::from(transform.a) * f64::from(transform.d)
                        - f64::from(transform.b) * f64::from(transform.c);
                    check(
                        matrix.into_iter().all(f32::is_finite)
                            && determinant.is_finite()
                            && determinant != 0.0,
                        index,
                        "transform matrix",
                    )?;
                    active_transform = Some(*transform);
                }
                DrawOp::EndTransform => {
                    check(
                        active_transform.take().is_some(),
                        index,
                        "unmatched transform end",
                    )?;
                }
                DrawOp::Image(image) => {
                    validate_rect(image.rect, page.size.height).map_err(|field| {
                        PdfError::InvalidGeometry {
                            page: page_index,
                            op: index,
                            field,
                        }
                    })?;
                }
                DrawOp::GlyphText(text) => {
                    if text.text.source.len() > o2p_fonts::MAX_TEXT_BYTES
                        || text
                            .text
                            .runs
                            .iter()
                            .map(|r| r.glyphs.len())
                            .fold(0usize, usize::saturating_add)
                            > o2p_fonts::MAX_GLYPHS
                    {
                        return Err(PdfError::InvalidFont {
                            page: page_index,
                            op: op_index,
                            message: "shaped text limit exceeded".into(),
                        });
                    }
                    check(finite_point(text.origin), index, "text origin")?;
                    check(positive(text.text.font_size), index, "font size")?;
                    check(finite_point(text.text.advance), index, "text advance")?;
                    if let Some(character) = text.text.source.chars().find(|c| c.is_control()) {
                        return Err(PdfError::UnsupportedText {
                            page: page_index,
                            op: op_index,
                            character,
                        });
                    }
                    let mut missing_glyphs = Vec::new();
                    for run in &text.text.runs {
                        check(finite_point(run.origin), index, "glyph run origin")?;
                        let mut x = f64::from(text.origin.x) + f64::from(run.origin.x);
                        let mut y = f64::from(text.origin.y) + f64::from(run.origin.y);
                        for glyph in &run.glyphs {
                            check(
                                finite_point(glyph.offset) && finite_point(glyph.advance),
                                index,
                                "glyph position",
                            )?;
                            check(
                                ((x + f64::from(glyph.offset.x)) as f32).is_finite()
                                    && ((f64::from(page.size.height)
                                        - y
                                        - f64::from(glyph.offset.y))
                                        as f32)
                                        .is_finite(),
                                index,
                                "glyph position",
                            )?;
                            x += f64::from(glyph.advance.x);
                            y += f64::from(glyph.advance.y);
                            check(
                                (x as f32).is_finite() && (y as f32).is_finite(),
                                index,
                                "glyph advance",
                            )?;
                            if glyph.id == 0 {
                                if let Some(source) = text.text.source.get(glyph.source.clone()) {
                                    for character in source.chars() {
                                        if !fallback {
                                            return Err(PdfError::UnsupportedText {
                                                page: page_index,
                                                op: op_index,
                                                character,
                                            });
                                        }
                                        if !missing_glyphs.contains(&character) {
                                            missing_glyphs.push(character);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !missing_glyphs.is_empty() {
                        warnings.push(PdfWarning {
                            page: page_index,
                            op: op_index,
                            missing_glyphs,
                            kind: PdfWarningKind::EmbeddedMissingGlyph,
                        });
                    }
                }
                DrawOp::Text(text) => {
                    check(finite_point(text.origin), index, "text origin")?;
                    check(
                        (page.size.height - text.origin.y).is_finite(),
                        index,
                        "text baseline",
                    )?;
                    check(positive(text.font_size), index, "font size")?;
                    let mut missing_glyphs = vec![];
                    let mut seen = std::collections::BTreeSet::new();
                    for character in text.text.chars() {
                        if text::encode(character).is_none() {
                            if character.is_control() || !fallback {
                                return Err(PdfError::UnsupportedText {
                                    page: page_index,
                                    op: op_index,
                                    character,
                                });
                            }
                            if seen.insert(character) {
                                missing_glyphs.push(character);
                            }
                        }
                    }
                    if !missing_glyphs.is_empty() {
                        warnings.push(PdfWarning {
                            kind: PdfWarningKind::LegacyMissingGlyph,
                            page: page_index,
                            op: op_index,
                            missing_glyphs,
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
        check(
            active_transform.is_none(),
            Some(page.ops.len()),
            "unclosed transform scope",
        )?;
    }
    Ok(warnings)
}

// Existing primitives already use bottom-left PDF coordinates. Conjugate the
// generic matrix by P(x,y)=(x,H-y): P * M * P, without changing text/image code.
fn pdf_transform(t: Transform, page_height: f32) -> [f32; 6] {
    let h = f64::from(page_height);
    [
        t.a,
        -t.b,
        -t.c,
        t.d,
        (f64::from(t.c) * h + f64::from(t.e)) as f32,
        (h * (1.0 - f64::from(t.d)) - f64::from(t.f)) as f32,
    ]
}

fn transformed_geometry_is_finite(op: &DrawOp, transform: Transform, height: f32) -> bool {
    let point = |p: Point| {
        let p = transform.apply(p);
        finite_point(p) && (height - p.y).is_finite()
    };
    let rect = |r: o2p_core::Rect| {
        [
            r.origin,
            Point {
                x: r.origin.x + r.size.width,
                y: r.origin.y,
            },
            Point {
                x: r.origin.x,
                y: r.origin.y + r.size.height,
            },
            Point {
                x: r.origin.x + r.size.width,
                y: r.origin.y + r.size.height,
            },
        ]
        .into_iter()
        .all(point)
    };
    match op {
        DrawOp::BeginTransform(_) | DrawOp::EndTransform => true,
        DrawOp::Rect(r) => rect(r.rect),
        DrawOp::StrokeRect(r) => rect(r.rect),
        DrawOp::Ellipse(r) => rect(r.bounds),
        DrawOp::StrokeEllipse(r) => rect(r.bounds),
        DrawOp::Image(r) => rect(r.rect),
        DrawOp::Line(l) => point(l.start) && point(l.end),
        DrawOp::Text(t) => point(t.origin),
        DrawOp::GlyphText(t) => {
            point(t.origin)
                && t.text.runs.iter().all(|run| {
                    let mut x = f64::from(t.origin.x) + f64::from(run.origin.x);
                    let mut y = f64::from(t.origin.y) + f64::from(run.origin.y);
                    run.glyphs.iter().all(|glyph| {
                        let valid = point(Point {
                            x: (x + f64::from(glyph.offset.x)) as f32,
                            y: (y + f64::from(glyph.offset.y)) as f32,
                        });
                        x += f64::from(glyph.advance.x);
                        y += f64::from(glyph.advance.y);
                        valid
                    })
                })
        }
    }
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
