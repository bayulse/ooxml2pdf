//! The existing unwrapped layout policy, measured with configured shaped fonts.
use crate::{Presentation, TextAlignment, TextBox, TextStyle};
use o2p_core::{DisplayList, DrawOp, GlyphTextOp, Point};
use o2p_fonts::{FontError, FontRequest, FontResolver, FontWarning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextLayoutWarningKind {
    Font(FontWarning),
    Overflow,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLayoutWarning {
    pub page: usize,
    pub text_box: usize,
    pub kind: TextLayoutWarningKind,
}
impl std::fmt::Display for TextLayoutWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "page {}, text box {}: {:?}",
            self.page, self.text_box, self.kind
        )
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct FontDisplayReport {
    pub display_list: DisplayList,
    pub warnings: Vec<TextLayoutWarning>,
}
impl Presentation {
    /// Resolve and shape before layout/DisplayList construction. Callers retain
    /// the existing parsing diagnostics in `warnings()` alongside this report.
    pub fn to_display_list_with_fonts(
        &self,
        resolver: &dyn FontResolver,
    ) -> Result<FontDisplayReport, FontError> {
        let mut report = FontDisplayReport {
            display_list: DisplayList::default(),
            warnings: Vec::new(),
        };
        if self.slide_contents().is_empty() {
            report.display_list = self.to_blank_display_list();
            return Ok(report);
        }
        for (page_index, slide) in self.slide_contents().iter().enumerate() {
            let mut text_box = 0;
            let page = slide.to_page_with_text(self.size().to_points(), |text| {
                let (ops, warnings, overflow) = text.layout_with_fonts(resolver)?;
                report
                    .warnings
                    .extend(warnings.into_iter().map(|w| TextLayoutWarning {
                        page: page_index,
                        text_box,
                        kind: TextLayoutWarningKind::Font(w),
                    }));
                if overflow {
                    report.warnings.push(TextLayoutWarning {
                        page: page_index,
                        text_box,
                        kind: TextLayoutWarningKind::Overflow,
                    });
                }
                text_box += 1;
                Ok::<_, FontError>(ops)
            })?;
            report.display_list.pages.push(page);
        }
        Ok(report)
    }
}
impl TextBox {
    fn layout_with_fonts(
        &self,
        resolver: &dyn FontResolver,
    ) -> Result<(Vec<DrawOp>, Vec<FontWarning>, bool), FontError> {
        let mut ops = Vec::new();
        let mut warnings = Vec::new();
        let mut top = f64::from(self.rect.origin.y) + f64::from(self.insets.top);
        let left = f64::from(self.rect.origin.x) + f64::from(self.insets.left);
        let width = (f64::from(self.rect.size.width)
            - f64::from(self.insets.left)
            - f64::from(self.insets.right))
        .max(0.0);
        let bottom = f64::from(self.rect.origin.y) + f64::from(self.rect.size.height)
            - f64::from(self.insets.bottom);
        let mut overflow = false;
        for paragraph in &self.paragraphs {
            let mut lines: Vec<Vec<(&str, &TextStyle)>> = vec![vec![]];
            for run in &paragraph.runs {
                for (index, segment) in run.text.split('\n').enumerate() {
                    if index > 0 {
                        lines.push(vec![]);
                    }
                    if let Some(line) = lines.last_mut() {
                        line.push((segment, &run.style));
                    }
                }
            }
            let last = lines.len() - 1;
            for (index, line) in lines.into_iter().enumerate() {
                let mut size = line
                    .iter()
                    .filter(|(s, _)| !s.is_empty())
                    .map(|(_, s)| f64::from(s.font_size))
                    .fold(0.0, f64::max);
                if size == 0.0 {
                    size = f64::from(paragraph.end_style.font_size);
                }
                if index == last && line.iter().all(|(s, _)| s.is_empty()) {
                    size = size.max(f64::from(paragraph.end_style.font_size));
                }
                let mut shaped = Vec::new();
                let mut line_width = 0.0;
                for (text, style) in line {
                    if text.is_empty() {
                        continue;
                    }
                    let request = FontRequest {
                        family: style.family.clone().unwrap_or_else(|| "sans-serif".into()),
                        weight: if style.bold { 700 } else { 400 },
                        italic: style.italic,
                    };
                    let report = o2p_fonts::shape(resolver, text, &request, style.font_size)?;
                    for warning in report.warnings {
                        if !warnings.contains(&warning) {
                            warnings.push(warning);
                        }
                    }
                    line_width += f64::from(report.text.advance.x);
                    shaped.push((report.text, style.color));
                }
                let mut x = left
                    + match paragraph.alignment {
                        TextAlignment::Left => 0.0,
                        TextAlignment::Center => (width - line_width) / 2.0,
                        TextAlignment::Right => width - line_width,
                    };
                let baseline = top + size;
                overflow |= line_width > width || baseline > bottom || top + size * 1.2 > bottom;
                for (text, color) in shaped {
                    let advance = text.advance.x;
                    ops.push(DrawOp::GlyphText(GlyphTextOp {
                        origin: Point {
                            x: x as f32,
                            y: baseline as f32,
                        },
                        text,
                        color,
                    }));
                    x += f64::from(advance);
                }
                top += size * 1.2;
            }
        }
        Ok((ops, warnings, overflow))
    }
}
