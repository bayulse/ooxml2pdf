//! Format-independent, whitespace-based wrapping of explicitly styled Unicode.
//! Fonts measure and shape; this crate chooses lines and positions them. There
//! are no Office semantics, PDF commands, clipping, or font discovery here.

use o2p_core::{Color, GlyphTextOp, Point, Rect, ShapedText};
use o2p_fonts::{FontError, FontRequest, FontResolver, FontWarning, MAX_TEXT_BYTES};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// A paragraph has a bounded shaping/layout workload, including explicit breaks.
pub const MAX_PARAGRAPH_BYTES: usize = MAX_TEXT_BYTES;
pub const MAX_PARAGRAPHS: usize = 16_000;
pub const MAX_TEXT_BOX_BYTES: usize = 1_048_576;
pub const MAX_TEXT_BOX_RUNS: usize = 65_536;

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub font: FontRequest,
    pub font_size: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub runs: Vec<TextRun>,
    pub alignment: Alignment,
    /// Empty paragraphs and trailing explicit breaks occupy a line of this size.
    pub empty_font_size: f32,
}

/// A measured line, including blank lines. Runs share this baseline and retain
/// their own font size, color and resolved font resources.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub baseline: Point,
    pub width: f64,
    pub height: f64,
    pub runs: Vec<GlyphTextOp>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutReport {
    pub lines: Vec<Line>,
    pub warnings: Vec<FontWarning>,
    /// Horizontal or vertical overflow. All lines remain present, without clipping.
    pub overflow: bool,
}

struct Fragment<'a> {
    range: Range<usize>,
    style: &'a TextRun,
    shaped: ShapedText,
}

struct Atom {
    end: usize,
    advance: f64,
    whitespace: bool,
}

/// Lay out within an already-inset, top-left rectangle, in points. Explicit
/// newlines are mandatory breaks. Soft breaks prefer whitespace; unbroken tokens
/// fall back to boundaries shared by graphemes and all original shaping clusters.
/// Every iteration consumes at least one atom, even for zero-width boxes.
pub fn layout(
    paragraphs: &[Paragraph],
    bounds: Rect,
    resolver: &dyn FontResolver,
) -> Result<LayoutReport, FontError> {
    if !bounds.origin.x.is_finite()
        || !bounds.origin.y.is_finite()
        || !bounds.size.width.is_finite()
        || !bounds.size.height.is_finite()
        || bounds.size.width < 0.0
        || bounds.size.height < 0.0
    {
        return Err(FontError::InvalidRequest);
    }
    if paragraphs.len() > MAX_PARAGRAPHS {
        return Err(FontError::LimitExceeded("layout paragraphs"));
    }
    let mut total = 0usize;
    let mut run_count = 0;
    for paragraph in paragraphs {
        // Empty runs must not evade workload bounds.
        if paragraph.runs.len() > MAX_PARAGRAPH_BYTES {
            return Err(FontError::LimitExceeded("layout runs"));
        }
        run_count += paragraph.runs.len();
        if run_count > MAX_TEXT_BOX_RUNS {
            return Err(FontError::LimitExceeded("layout text box runs"));
        }
        let bytes = paragraph.runs.iter().try_fold(0usize, |sum, run| {
            sum.checked_add(run.text.len())
                .ok_or(FontError::LimitExceeded("layout paragraph bytes"))
        })?;
        if bytes > MAX_PARAGRAPH_BYTES {
            return Err(FontError::LimitExceeded("layout paragraph bytes"));
        }
        total += bytes;
        if total > MAX_TEXT_BOX_BYTES {
            return Err(FontError::LimitExceeded("layout text box bytes"));
        }
        if !valid_size(paragraph.empty_font_size)
            || paragraph.runs.iter().any(|r| !valid_size(r.font_size))
        {
            return Err(FontError::InvalidRequest);
        }
    }
    let mut report = LayoutReport {
        lines: Vec::new(),
        warnings: Vec::new(),
        overflow: false,
    };
    let mut top = f64::from(bounds.origin.y);
    for paragraph in paragraphs {
        let mut reshaped_bytes = 0;
        let mut hard_lines = vec![Vec::new()];
        for run in &paragraph.runs {
            for (index, segment) in run.text.split('\n').enumerate() {
                if index != 0 {
                    hard_lines.push(Vec::new());
                }
                if let Some(line) = hard_lines.last_mut() {
                    line.push((segment, run));
                }
            }
        }
        for hard_line in hard_lines {
            let mut source = String::new();
            let mut fragments = Vec::new();
            for (text, style) in hard_line {
                if text.is_empty() {
                    continue;
                }
                let start = source.len();
                source.push_str(text);
                let shaped = o2p_fonts::shape(resolver, text, &style.font, style.font_size)?;
                warnings(&mut report.warnings, shaped.warnings);
                fragments.push(Fragment {
                    range: start..source.len(),
                    style,
                    shaped: shaped.text,
                });
            }
            if source.is_empty() {
                append_line(&mut report, Vec::new(), paragraph, bounds, &mut top)?;
                continue;
            }
            let atoms = atoms(&source, &fragments);
            let mut first = 0;
            while first < atoms.len() {
                let start = if first == 0 { 0 } else { atoms[first - 1].end };
                let mut last = first;
                let mut measured = 0.0;
                let mut whitespace = None;
                while last < atoms.len() {
                    let next = measured + atoms[last].advance;
                    // The shaper rounds total advance to f32. Permit only its
                    // rounding error during estimation; final widths are exact.
                    let slack =
                        f64::from(f32::EPSILON) * f64::from(bounds.size.width).max(1.0) * 4.0;
                    if next > f64::from(bounds.size.width) + slack {
                        break;
                    }
                    measured = next;
                    last += 1;
                    if atoms[last - 1].whitespace {
                        whitespace = Some(last);
                    }
                }
                if last < atoms.len() {
                    last = whitespace.unwrap_or(last);
                }
                last = last.max(first + 1);
                // Re-shape at the actual line edges: Arabic joining, ligatures,
                // kerning and bidi may change after a cut. Never emit a too-wide
                // multi-atom line merely because its original estimate fitted.
                let runs = loop {
                    let runs = shape_range(
                        &fragments,
                        start..atoms[last - 1].end,
                        resolver,
                        &mut report.warnings,
                        &mut reshaped_bytes,
                    )?;
                    let width: f64 = runs.iter().map(|r| f64::from(r.text.advance.x)).sum();
                    if width <= f64::from(bounds.size.width) || last == first + 1 {
                        break runs;
                    }
                    last = (first + 1..last)
                        .rev()
                        .find(|&end| atoms[end - 1].whitespace)
                        .unwrap_or(last - 1);
                };
                append_line(&mut report, runs, paragraph, bounds, &mut top)?;
                first = last;
            }
        }
    }
    Ok(report)
}

fn valid_size(size: f32) -> bool {
    size.is_finite() && size > 0.0
}

fn warnings(destination: &mut Vec<FontWarning>, incoming: Vec<FontWarning>) {
    for warning in incoming {
        if !destination.contains(&warning) {
            destination.push(warning);
        }
    }
}

fn atoms(source: &str, fragments: &[Fragment<'_>]) -> Vec<Atom> {
    // A style/font/direction boundary is not automatically a break opportunity.
    // Intersect full-text grapheme boundaries with the glyph cluster intervals.
    let mut glyphs: Vec<_> = fragments
        .iter()
        .flat_map(|fragment| {
            fragment.shaped.runs.iter().flat_map(move |run| {
                run.glyphs.iter().map(move |glyph| {
                    (
                        fragment.range.start + glyph.source.start,
                        fragment.range.start + glyph.source.end,
                        f64::from(glyph.advance.x),
                    )
                })
            })
        })
        .collect();
    glyphs.sort_by_key(|&(start, end, _)| (start, end));
    let mut cursor = 0;
    let mut covered_end = 0;
    let mut boundaries = vec![0];
    for boundary in source.grapheme_indices(true).map(|(i, _)| i).skip(1) {
        while cursor < glyphs.len() && glyphs[cursor].0 < boundary {
            covered_end = covered_end.max(glyphs[cursor].1);
            cursor += 1;
        }
        if covered_end <= boundary {
            boundaries.push(boundary);
        }
    }
    boundaries.push(source.len());
    let mut atoms: Vec<_> = boundaries
        .windows(2)
        .map(|pair| Atom {
            end: pair[1],
            advance: 0.0,
            whitespace: source[pair[0]..pair[1]]
                .chars()
                .all(|c| c.is_whitespace() && !matches!(c, '\u{00a0}' | '\u{202f}')),
        })
        .collect();
    for (start, _, advance) in glyphs {
        let index = atoms.partition_point(|atom| atom.end <= start);
        if let Some(atom) = atoms.get_mut(index) {
            atom.advance += advance;
        }
    }
    atoms
}

fn shape_range(
    fragments: &[Fragment<'_>],
    range: Range<usize>,
    resolver: &dyn FontResolver,
    diagnostics: &mut Vec<FontWarning>,
    reshaped_bytes: &mut usize,
) -> Result<Vec<GlyphTextOp>, FontError> {
    let first = fragments.partition_point(|f| f.range.end <= range.start);
    let mut runs = Vec::new();
    for fragment in &fragments[first..] {
        if fragment.range.start >= range.end {
            break;
        }
        let start = range.start.max(fragment.range.start) - fragment.range.start;
        let end = range.end.min(fragment.range.end) - fragment.range.start;
        let text = if start == 0 && end == fragment.shaped.source.len() {
            fragment.shaped.clone()
        } else {
            *reshaped_bytes += end - start;
            if *reshaped_bytes > MAX_TEXT_BOX_BYTES {
                return Err(FontError::LimitExceeded("layout reshaping bytes"));
            }
            let source = fragment
                .shaped
                .source
                .get(start..end)
                .ok_or_else(|| FontError::Shaping("invalid layout cluster boundary".into()))?;
            let shaped = o2p_fonts::shape(
                resolver,
                source,
                &fragment.style.font,
                fragment.style.font_size,
            )?;
            warnings(diagnostics, shaped.warnings);
            shaped.text
        };
        runs.push(GlyphTextOp {
            origin: Point { x: 0.0, y: 0.0 },
            text,
            color: fragment.style.color,
        });
    }
    Ok(runs)
}

fn append_line(
    report: &mut LayoutReport,
    mut runs: Vec<GlyphTextOp>,
    paragraph: &Paragraph,
    bounds: Rect,
    top: &mut f64,
) -> Result<(), FontError> {
    let size = runs
        .iter()
        .map(|run| f64::from(run.text.font_size))
        .reduce(f64::max)
        .unwrap_or(f64::from(paragraph.empty_font_size));
    let width: f64 = runs.iter().map(|run| f64::from(run.text.advance.x)).sum();
    let offset = match paragraph.alignment {
        Alignment::Left => 0.0,
        Alignment::Center => (f64::from(bounds.size.width) - width) / 2.0,
        Alignment::Right => f64::from(bounds.size.width) - width,
    };
    let mut x = f64::from(bounds.origin.x) + offset;
    let baseline = *top + size;
    let height = size * 1.2;
    let next = *top + height;
    if !width.is_finite()
        || !(x as f32).is_finite()
        || !(baseline as f32).is_finite()
        || !(next as f32).is_finite()
    {
        return Err(FontError::InvalidRequest);
    }
    let line_baseline = Point {
        x: x as f32,
        y: baseline as f32,
    };
    for run in &mut runs {
        if !(x as f32).is_finite() {
            return Err(FontError::InvalidRequest);
        }
        run.origin = Point {
            x: x as f32,
            y: baseline as f32,
        };
        x += f64::from(run.text.advance.x);
    }
    report.overflow |= width > f64::from(bounds.size.width)
        || next > f64::from(bounds.origin.y) + f64::from(bounds.size.height);
    report.lines.push(Line {
        baseline: line_baseline,
        width,
        height,
        runs,
    });
    *top = next;
    Ok(())
}
