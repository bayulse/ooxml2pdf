use crate::{
    FontError, FontFace, FontRequest, FontResolver, FontWarning, MAX_GLYPHS, MAX_TEXT_BYTES,
};
use o2p_core::{Glyph, GlyphRun, Point, ShapedText};
use std::{collections::BTreeSet, ops::Range, sync::Arc};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeReport {
    pub text: ShapedText,
    pub warnings: Vec<FontWarning>,
}
struct Segment {
    range: Range<usize>,
    font: Arc<FontFace>,
    script: Script,
}

/// Shape one unwrapped horizontal semantic run, with Unicode bidi/script
/// segmentation and grapheme-preserving fallback. Positions use points.
pub fn shape(
    resolver: &dyn FontResolver,
    source: &str,
    request: &FontRequest,
    font_size: f32,
) -> Result<ShapeReport, FontError> {
    if source.len() > MAX_TEXT_BYTES {
        return Err(FontError::LimitExceeded("text bytes per run"));
    }
    if !font_size.is_finite()
        || font_size <= 0.0
        || request.family.trim().is_empty()
        || request.family.len() > 256
        || !(1..=1000).contains(&request.weight)
        || source.chars().any(char::is_control)
    {
        return Err(FontError::InvalidRequest);
    }
    let mut warnings = Vec::new();
    let mut runs = Vec::new();
    let mut pen = 0.0_f64;
    let mut glyph_count = 0;
    let bidi = unicode_bidi::BidiInfo::new(source, None);
    for paragraph in &bidi.paragraphs {
        let (levels, visual) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for range in visual {
            let rtl = levels[range.start].is_rtl();
            let clusters: Vec<_> = source[range.clone()].grapheme_indices(true).collect();
            let mut scripts: Vec<_> = clusters
                .iter()
                .map(|(_, s)| {
                    s.chars().map(|c| c.script()).find(|s| {
                        !matches!(s, Script::Common | Script::Inherited | Script::Unknown)
                    })
                })
                .collect();
            let mut previous = None;
            for script in &mut scripts {
                if script.is_none() {
                    *script = previous;
                } else {
                    previous = *script;
                }
            }
            let mut next = Some(Script::Latin);
            for script in scripts.iter_mut().rev() {
                if script.is_none() {
                    *script = next;
                } else {
                    next = *script;
                }
            }
            let mut segments: Vec<Segment> = Vec::new();
            for ((offset, cluster), script) in clusters.into_iter().zip(scripts) {
                let selection = resolver.resolve(request, cluster)?;
                for warning in selection.warnings {
                    if !warnings.contains(&warning) {
                        warnings.push(warning);
                    }
                }
                let start = range.start + offset;
                let end = start + cluster.len();
                let script = script.unwrap_or(Script::Latin);
                if let Some(last) = segments.last_mut() {
                    if Arc::ptr_eq(&last.font, &selection.font) && last.script == script {
                        last.range.end = end;
                        continue;
                    }
                }
                segments.push(Segment {
                    range: start..end,
                    font: selection.font,
                    script,
                });
            }
            if rtl {
                segments.reverse();
            }
            for segment in segments {
                let mut buffer = harfrust::Buffer::new();
                for (offset, c) in source[segment.range.clone()].char_indices() {
                    buffer.push(c as u32, (segment.range.start + offset) as u32);
                }
                buffer.set_direction(if rtl {
                    harfrust::Direction::RightToLeft
                } else {
                    harfrust::Direction::LeftToRight
                });
                buffer.set_script(segment.script.short_name().parse().ok());
                buffer.set_pre_context(&source[..segment.range.start]);
                buffer.set_post_context(&source[segment.range.end..]);
                let shaper = harfrust::ShaperFont::new(&segment.font.shaper);
                harfrust::shape(&shaper, &mut buffer, harfrust::ShapeOptions::default())
                    .map_err(|e| FontError::Shaping(e.to_string()))?;
                glyph_count += buffer.len();
                if glyph_count > MAX_GLYPHS {
                    return Err(FontError::LimitExceeded("shaped glyphs"));
                }
                let starts: BTreeSet<_> = buffer
                    .glyph_infos()
                    .iter()
                    .map(|g| g.cluster as usize)
                    .chain([segment.range.end])
                    .collect();
                let scale = f64::from(font_size) / f64::from(segment.font.metadata().units_per_em);
                let mut glyphs = Vec::with_capacity(buffer.len());
                let origin = Point {
                    x: pen as f32,
                    y: 0.0,
                };
                for (info, position) in buffer.glyph_infos().iter().zip(buffer.glyph_positions()) {
                    let start = info.cluster as usize;
                    let end = starts
                        .range((start + 1)..)
                        .next()
                        .copied()
                        .unwrap_or(segment.range.end);
                    if start < segment.range.start
                        || end > segment.range.end
                        || source.get(start..end).is_none()
                    {
                        return Err(FontError::Shaping("invalid source cluster".into()));
                    }
                    let id = u16::try_from(info.glyph_id).map_err(|_| FontError::InvalidFont)?;
                    if id == 0 {
                        for c in source[start..end]
                            .chars()
                            .filter(|&c| !crate::registry::ignorable(c))
                        {
                            let warning = FontWarning::MissingGlyph { character: c };
                            if !warnings.contains(&warning) {
                                warnings.push(warning);
                            }
                        }
                    }
                    let advance = Point {
                        x: (f64::from(position.x_advance) * scale) as f32,
                        y: (-f64::from(position.y_advance) * scale) as f32,
                    };
                    let offset = Point {
                        x: (f64::from(position.x_offset) * scale) as f32,
                        y: (-f64::from(position.y_offset) * scale) as f32,
                    };
                    pen += f64::from(advance.x);
                    if !pen.is_finite()
                        || !advance.x.is_finite()
                        || !advance.y.is_finite()
                        || !offset.x.is_finite()
                        || !offset.y.is_finite()
                        || !(pen as f32).is_finite()
                    {
                        return Err(FontError::InvalidRequest);
                    }
                    glyphs.push(Glyph {
                        id,
                        advance,
                        offset,
                        source: start..end,
                    });
                }
                runs.push(GlyphRun {
                    font: segment.font.resource().clone(),
                    origin,
                    glyphs,
                });
            }
        }
    }
    Ok(ShapeReport {
        text: ShapedText {
            source: source.into(),
            font_size,
            runs,
            advance: Point {
                x: pen as f32,
                y: 0.0,
            },
        },
        warnings,
    })
}
