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
    Shadowed {
        shadow: o2p_core::Shadow,
        element: Box<Shape>,
    },
    RoundedRectangle {
        rect: Rect,
        radius: f32,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
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
    pub(crate) placeholders: Vec<crate::Placeholder>,
}

impl Slide {
    pub fn background(&self) -> Option<Fill> {
        self.background
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }
    /// Effective placeholder identities/properties, in slide XML order.
    pub fn placeholders(&self) -> &[crate::Placeholder] {
        &self.placeholders
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
            Shape::Shadowed { shadow, element } => {
                let (geometry, fill, stroke_width) = match element.as_ref() {
                    Shape::Rectangle { rect, fill, stroke } => (
                        o2p_core::ShadowGeometry::Rectangle(*rect),
                        matches!(fill, Some(Fill::Solid(_))),
                        stroke.map_or(0.0, |s| s.width),
                    ),
                    Shape::RoundedRectangle {
                        rect,
                        radius,
                        fill,
                        stroke,
                    } => (
                        o2p_core::ShadowGeometry::RoundedRectangle {
                            rect: *rect,
                            radius: *radius,
                        },
                        matches!(fill, Some(Fill::Solid(_))),
                        stroke.map_or(0.0, |s| s.width),
                    ),
                    Shape::Ellipse {
                        bounds,
                        fill,
                        stroke,
                    } => (
                        o2p_core::ShadowGeometry::Ellipse(*bounds),
                        matches!(fill, Some(Fill::Solid(_))),
                        stroke.map_or(0.0, |s| s.width),
                    ),
                    Shape::Image(image) => {
                        (o2p_core::ShadowGeometry::Image(image.clone()), true, 0.0)
                    }
                    _ => {
                        append_shapes(std::slice::from_ref(element), ops, layout)?;
                        continue;
                    }
                };
                if fill || stroke_width > 0.0 {
                    ops.push(DrawOp::Shadow(o2p_core::ShadowOp {
                        geometry,
                        fill,
                        stroke_width,
                        shadow: *shadow,
                    }));
                }
                append_shapes(std::slice::from_ref(element), ops, layout)?;
                continue;
            }
            Shape::RoundedRectangle {
                rect,
                radius,
                fill,
                stroke,
            } => {
                if let Some(Fill::Solid(color)) = fill {
                    ops.push(DrawOp::RoundedRect(o2p_core::RoundedRectOp {
                        rect: *rect,
                        radius: *radius,
                        color: *color,
                    }));
                }
                if let Some(stroke) = stroke {
                    ops.push(DrawOp::StrokeRoundedRect(o2p_core::StrokeRoundedRectOp {
                        rect: *rect,
                        radius: *radius,
                        color: stroke.color,
                        width: stroke.width,
                    }));
                }
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
    /// Absent means configured sans-serif after supported placeholder inheritance.
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
    pub bullet: Option<o2p_core::Bullet>,
    pub format: o2p_core::ParagraphFormat,
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
    pub vertical_alignment: o2p_core::VerticalAlignment,
    /// Include space-before/after at the outer edges of the complete text block.
    pub include_edge_spacing: bool,
    pub rect: Rect,
    pub insets: TextInsets,
    pub paragraphs: Vec<Paragraph>,
}
impl TextBox {
    /// Fixed-font compatibility layout: explicit breaks and effective paragraph
    /// geometry/spacing/anchor are handled by the generic layout crate.
    pub(crate) fn layout(&self) -> (Vec<o2p_core::TextOp>, bool) {
        o2p_layout::layout_unwrapped(
            &self.layout_paragraphs(),
            self.inner_rect(),
            self.vertical_alignment,
        )
    }
}
