//! Owned, normalized slide content and its format-independent drawing adapter.
use o2p_core::{
    Color, DrawOp, EllipseOp, ImageOp, LineOp, Page, Point, Rect, RectOp, Size, StrokeEllipseOp,
    StrokeRectOp, Transform,
};

/// Explicit paint; absent/unresolved paint is represented by `None` on a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    None,
    Solid(Color),
}

/// A resolved solid outline in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

/// Supported explicit geometry in points, without XML or PDF types.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// One transformed element: its geometry and optional text share a scope.
    /// This does not represent a DrawingML group or inherited coordinate space.
    Transformed {
        transform: Transform,
        element: Vec<Shape>,
    },
    /// Text paints immediately after its associated vector geometry.
    TextBox(TextBox),
    /// An embedded raster picture, in the same paint sequence as vector shapes.
    Image(ImageOp),
    Rectangle {
        rect: Rect,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
    Ellipse {
        bounds: Rect,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
    Line {
        start: Point,
        end: Point,
        stroke: Option<Stroke>,
    },
}

/// Content of one slide. Shapes preserve their XML painting order.
#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
    pub(crate) background: Option<Fill>,
    pub(crate) shapes: Vec<Shape>,
}

impl Slide {
    pub fn background(&self) -> Option<Fill> {
        self.background
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub(crate) fn to_page(&self, size: Size) -> Page {
        match self.to_page_with_text(size, |text| {
            Ok::<_, std::convert::Infallible>(
                text.layout().0.into_iter().map(DrawOp::Text).collect(),
            )
        }) {
            Ok(page) => page,
            Err(never) => match never {},
        }
    }

    pub(crate) fn to_page_with_text<E>(
        &self,
        size: Size,
        mut layout: impl FnMut(&TextBox) -> Result<Vec<DrawOp>, E>,
    ) -> Result<Page, E> {
        let mut ops = vec![];
        if let Some(Fill::Solid(color)) = self.background {
            ops.push(DrawOp::Rect(RectOp {
                rect: Rect {
                    origin: Point { x: 0.0, y: 0.0 },
                    size,
                },
                color,
            }));
        }
        append_shapes(&self.shapes, &mut ops, &mut layout)?;
        Ok(Page { size, ops })
    }
}

fn append_shapes<E>(
    shapes: &[Shape],
    ops: &mut Vec<DrawOp>,
    layout: &mut impl FnMut(&TextBox) -> Result<Vec<DrawOp>, E>,
) -> Result<(), E> {
    for shape in shapes {
        let (rect, fill, stroke) = match shape {
            Shape::Transformed { transform, element } => {
                ops.push(DrawOp::BeginTransform(*transform));
                append_shapes(element, ops, layout)?;
                ops.push(DrawOp::EndTransform);
                continue;
            }
            Shape::TextBox(text) => {
                ops.extend(layout(text)?);
                continue;
            }
            Shape::Image(image) => {
                ops.push(DrawOp::Image(image.clone()));
                continue;
            }
            Shape::Rectangle { rect, fill, stroke } => (rect, fill, stroke),
            Shape::Ellipse {
                bounds,
                fill,
                stroke,
            } => {
                if let Some(Fill::Solid(color)) = fill {
                    ops.push(DrawOp::Ellipse(EllipseOp {
                        bounds: *bounds,
                        color: *color,
                    }));
                }
                if let Some(stroke) = stroke {
                    ops.push(DrawOp::StrokeEllipse(StrokeEllipseOp {
                        bounds: *bounds,
                        color: stroke.color,
                        width: stroke.width,
                    }));
                }
                continue;
            }
            Shape::Line { start, end, stroke } => {
                if let Some(stroke) = stroke {
                    ops.push(DrawOp::Line(LineOp {
                        start: *start,
                        end: *end,
                        color: stroke.color,
                        width: stroke.width,
                    }));
                }
                continue;
            }
        };
        if let Some(Fill::Solid(color)) = fill {
            ops.push(DrawOp::Rect(RectOp {
                rect: *rect,
                color: *color,
            }));
        }
        if let Some(stroke) = stroke {
            ops.push(DrawOp::StrokeRect(StrokeRectOp {
                rect: *rect,
                color: stroke.color,
                width: stroke.width,
            }));
        }
    }
    Ok(())
}

/// Resolved basic run properties; all sizes use points.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Direct Latin typeface or resolved explicit Latin theme token.
    /// Absent means configured sans-serif; master/layout inheritance is unresolved.
    pub family: Option<String>,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: Color,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: None,
            font_size: 18.0,
            bold: false,
            italic: false,
            color: Color::BLACK,
        }
    }
}
impl TextStyle {
    fn font(&self) -> o2p_core::Font {
        o2p_core::Font::sans_serif(self.bold, self.italic)
    }
}

/// Literal Unicode; newline characters represent explicit line breaks.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub runs: Vec<TextRun>,
    pub alignment: TextAlignment,
    /// Used for blank paragraphs/trailing breaks; never restyles preceding runs.
    pub end_style: TextStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}
impl Default for TextInsets {
    fn default() -> Self {
        Self {
            left: 7.2,
            right: 7.2,
            top: 3.6,
            bottom: 3.6,
        }
    }
}

/// Shape transform and ordered paragraphs, independent of XML/PDF lifetimes.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub rect: Rect,
    pub insets: TextInsets,
    pub paragraphs: Vec<Paragraph>,
}
impl TextBox {
    /// Unwrapped lines, top aligned, with 1.2 times the largest size per line.
    /// Returns positioned core runs and whether text exceeds the inner box.
    pub(crate) fn layout(&self) -> (Vec<o2p_core::TextOp>, bool) {
        let mut ops = vec![];
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
                    if index != 0 {
                        lines.push(vec![]);
                    }
                    // Keep even empty segments: a break's direct size affects line height.
                    lines
                        .last_mut()
                        .expect("a line always exists")
                        .push((segment, &run.style));
                }
            }
            let last_line = lines.len() - 1;
            for (line_index, line) in lines.into_iter().enumerate() {
                let line_size =
                    if line_index == last_line && line.iter().all(|(text, _)| text.is_empty()) {
                        f64::from(paragraph.end_style.font_size)
                    } else {
                        line.iter()
                            .map(|(_, style)| f64::from(style.font_size))
                            .reduce(f64::max)
                            .unwrap_or(f64::from(paragraph.end_style.font_size))
                    };

                let line_width: f64 = line
                    .iter()
                    .map(|(text, style)| style.font().text_width(text, style.font_size))
                    .sum();
                let mut x = left
                    + match paragraph.alignment {
                        TextAlignment::Left => 0.0,
                        TextAlignment::Center => (width - line_width) / 2.0,
                        TextAlignment::Right => width - line_width,
                    };
                let baseline = top + line_size;
                overflow |= line_width > width || top + line_size * 1.2 > bottom;
                for (text, style) in line {
                    if !text.is_empty() {
                        ops.push(o2p_core::TextOp {
                            origin: Point {
                                x: x as f32,
                                y: baseline as f32,
                            },
                            text: text.into(),
                            font: style.font(),
                            font_size: style.font_size,
                            color: style.color,
                        });
                    }
                    x += style.font().text_width(text, style.font_size);
                }
                top += line_size * 1.2;
            }
        }
        (ops, overflow)
    }
}
