//! Format-independent text requests and effective paragraph geometry, in points.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontRequest {
    pub family: String,
    /// OpenType weight, 1 through 1000 (400 regular, 700 bold).
    pub weight: u16,
    pub italic: bool,
}
impl Default for FontRequest {
    fn default() -> Self {
        Self {
            family: "sans-serif".into(),
            weight: 400,
            italic: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bullet {
    pub glyph: String,
    pub font: FontRequest,
    pub font_size: f32,
    pub color: crate::Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParagraphSpacing {
    Points(f32),
    /// A multiple of the largest font size on the paragraph's boundary line.
    TextMultiple(f32),
}
impl Default for ParagraphSpacing {
    fn default() -> Self {
        Self::Points(0.0)
    }
}
impl ParagraphSpacing {
    pub fn points(self, text_size: f64) -> f64 {
        match self {
            Self::Points(value) => f64::from(value),
            Self::TextMultiple(value) => f64::from(value) * text_size,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParagraphFormat {
    pub margin_left: f32,
    pub margin_right: f32,
    /// Signed offset from the left margin: bullet origin, or first text line.
    /// Negative values produce a hanging indent. Continuations use the margin.
    pub first_line_indent: f32,
    pub space_before: ParagraphSpacing,
    pub space_after: ParagraphSpacing,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerticalAlignment {
    #[default]
    Top,
    Center,
    Bottom,
}
