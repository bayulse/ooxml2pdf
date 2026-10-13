//! Adapt PPTX semantics to generic measured text layout.
use crate::{Presentation, TextAlignment, TextBox};
use o2p_core::{DisplayList, DrawOp, Point, Rect, Size};
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
        let paragraphs = self.layout_paragraphs();
        let layout = o2p_layout::layout_text_box(
            &paragraphs,
            self.inner_rect(),
            self.vertical_alignment,
            resolver,
        )?;
        let ops = layout
            .lines
            .into_iter()
            .flat_map(|line| line.runs.into_iter().map(DrawOp::GlyphText))
            .collect();
        Ok((ops, layout.warnings, layout.overflow))
    }
    pub(crate) fn layout_paragraphs(&self) -> Vec<o2p_layout::Paragraph> {
        let mut paragraphs: Vec<o2p_layout::Paragraph> = self
            .paragraphs
            .iter()
            .map(|paragraph| o2p_layout::Paragraph {
                bullet: paragraph.bullet.clone(),
                format: paragraph.format,
                runs: paragraph
                    .runs
                    .iter()
                    .map(|run| o2p_layout::TextRun {
                        text: run.text.clone(),
                        font: FontRequest {
                            family: run
                                .style
                                .family
                                .clone()
                                .unwrap_or_else(|| "sans-serif".into()),
                            weight: if run.style.bold { 700 } else { 400 },
                            italic: run.style.italic,
                        },
                        font_size: run.style.font_size,
                        color: run.style.color,
                    })
                    .collect(),
                alignment: match paragraph.alignment {
                    TextAlignment::Left => o2p_layout::Alignment::Left,
                    TextAlignment::Center => o2p_layout::Alignment::Center,
                    TextAlignment::Right => o2p_layout::Alignment::Right,
                },
                empty_font_size: paragraph.end_style.font_size,
            })
            .collect();
        if !self.include_edge_spacing {
            if let Some(first) = paragraphs.first_mut() {
                first.format.space_before = Default::default();
            }
            if let Some(last) = paragraphs.last_mut() {
                last.format.space_after = Default::default();
            }
        }
        paragraphs
    }
    pub(crate) fn inner_rect(&self) -> Rect {
        Rect {
            origin: Point {
                x: (f64::from(self.rect.origin.x) + f64::from(self.insets.left)) as f32,
                y: (f64::from(self.rect.origin.y) + f64::from(self.insets.top)) as f32,
            },
            size: Size {
                width: (f64::from(self.rect.size.width)
                    - f64::from(self.insets.left)
                    - f64::from(self.insets.right))
                .max(0.0) as f32,
                height: (f64::from(self.rect.size.height)
                    - f64::from(self.insets.top)
                    - f64::from(self.insets.bottom))
                .max(0.0) as f32,
            },
        }
    }
}
