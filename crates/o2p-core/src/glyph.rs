//! Owned, already-shaped text. No parser, shaper, or PDF types are exposed.
use crate::{Color, Point};
use std::{ops::Range, sync::Arc};

/// Full encoded standalone sfnt font. Consumers and rendering boundaries validate
/// untrusted bytes; sharing a resource never copies the font program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontResource {
    pub bytes: Arc<[u8]>,
}
impl AsRef<[u8]> for FontResource {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// A glyph in visual paint order. Advances/offsets are points, y grows downward.
/// Several glyphs may share a UTF-8 source range, or one glyph may cover several
/// scalars. Source ranges are relative to `ShapedText::source`.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub id: u16,
    pub advance: Point,
    pub offset: Point,
    pub source: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    pub font: Arc<FontResource>,
    /// Relative baseline of this font/script/direction segment.
    pub origin: Point,
    pub glyphs: Vec<Glyph>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    /// Original logical Unicode, retained independently of visual glyph order.
    pub source: String,
    pub font_size: f32,
    pub runs: Vec<GlyphRun>,
    pub advance: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphTextOp {
    pub origin: Point,
    pub text: ShapedText,
    pub color: Color,
}
