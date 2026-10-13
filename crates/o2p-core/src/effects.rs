use crate::{Color, ImageOp, Point, Rect};

/// Circular corners in points; radius must be within half the shorter side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedRectOp {
    pub rect: Rect,
    pub radius: f32,
    pub color: Color,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeRoundedRectOp {
    pub rect: Rect,
    pub radius: f32,
    pub color: Color,
    pub width: f32,
}

/// Local-space shadow parameters. Opacity is in [0, 1]. Blur is a radius in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset: Point,
    pub blur_radius: f32,
    pub color: Color,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShadowGeometry {
    Rectangle(Rect),
    RoundedRectangle { rect: Rect, radius: f32 },
    Ellipse(Rect),
    Image(ImageOp),
}
impl ShadowGeometry {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rectangle(r) | Self::Ellipse(r) | Self::RoundedRectangle { rect: r, .. } => *r,
            Self::Image(i) => i.rect,
        }
    }
}
/// Paint an element's silhouette before its own paint, in the same transform scope.
/// Fill and stroke describe the silhouette, not the shadow's color.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowOp {
    pub geometry: ShadowGeometry,
    pub fill: bool,
    pub stroke_width: f32,
    pub shadow: Shadow,
}
