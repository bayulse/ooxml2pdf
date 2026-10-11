//! Owned, normalized slide content and its format-independent drawing adapter.
use o2p_core::{Color, DrawOp, Page, Point, Rect, RectOp, Size, StrokeRectOp};

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

/// Only axis-aligned rectangles are supported in this milestone.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rectangle {
        rect: Rect,
        fill: Option<Fill>,
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
        for shape in &self.shapes {
            let Shape::Rectangle { rect, fill, stroke } = shape;
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
        Page { size, ops }
    }
}
