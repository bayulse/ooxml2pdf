//! Format-independent, already-positioned drawing commands.
//!
//! All geometry uses points (1/72 inch), with the origin at the page's top-left,
//! x increasing rightward and y increasing downward. Operations paint in order.
//! This crate performs no layout, font discovery, shaping, or file I/O.

/// A position in points. Negative coordinates are allowed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Dimensions in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// A rectangle defined by its top-left corner and dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

/// An opaque sRGB color. Byte channels make every color value valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

/// A logical font choice, independent of a backend's font types.
///
/// Milestone 0 provides one regular sans-serif face. Font embedding and shaping
/// will require an explicit extension to this model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    SansSerif,
}

/// One positioned text run; `origin` is the left end of its baseline.
///
/// Text is Unicode in the model; individual backends report unsupported text.
/// Line breaks must be laid out into separate operations by the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct TextOp {
    pub origin: Point,
    pub text: String,
    pub font: Font,
    pub font_size: f32,
    pub color: Color,
}

/// A filled rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectOp {
    pub rect: Rect,
    pub color: Color,
}

/// A closed rectangle outline, centered on the rectangle's edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeRectOp {
    pub rect: Rect,
    pub color: Color,
    /// Positive width in points.
    pub width: f32,
}

/// A solid line with a positive width in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineOp {
    pub start: Point,
    pub end: Point,
    pub width: f32,
    pub color: Color,
}

/// An explicit drawing command. No Office or PDF semantics appear here.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    Text(TextOp),
    Rect(RectOp),
    StrokeRect(StrokeRectOp),
    Line(LineOp),
}

/// A page with positive dimensions and operations in painting order.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub size: Size,
    pub ops: Vec<DrawOp>,
}

/// Ordered pages forming a document. A renderer may reject an empty list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayList {
    pub pages: Vec<Page>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_channels_are_exact() {
        assert_eq!(Color::BLACK, Color::rgb(0, 0, 0));
        assert_eq!(Color::WHITE, Color::rgb(255, 255, 255));
        assert_eq!(Color::rgb(12, 34, 56).green, 34);
    }

    #[test]
    fn display_list_preserves_page_and_paint_order() {
        let rect = DrawOp::Rect(RectOp {
            rect: Rect {
                origin: Point { x: 10.0, y: 20.0 },
                size: Size {
                    width: 30.0,
                    height: 40.0,
                },
            },
            color: Color::WHITE,
        });
        let line = DrawOp::Line(LineOp {
            start: Point { x: 0.0, y: 0.0 },
            end: Point { x: 10.0, y: 10.0 },
            width: 1.0,
            color: Color::BLACK,
        });
        let display = DisplayList {
            pages: vec![
                Page {
                    size: Size {
                        width: 100.0,
                        height: 200.0,
                    },
                    ops: vec![rect.clone(), line.clone()],
                },
                Page {
                    size: Size {
                        width: 300.0,
                        height: 400.0,
                    },
                    ops: vec![],
                },
            ],
        };
        assert_eq!(display.pages[0].ops, vec![rect, line]);
        assert_eq!(display.pages[1].size.width, 300.0);
        assert_eq!(display.clone(), display);
    }
}
