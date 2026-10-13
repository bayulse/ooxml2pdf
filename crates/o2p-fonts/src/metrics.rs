use crate::FontError;

/// The table supplying a complete scalable horizontal line metric triplet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalMetricSource {
    Typographic,
    Hhea,
    Windows,
}

/// Signed font-unit values. Descender is at or below the baseline (<= 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontVerticalMetrics {
    pub units_per_em: u16,
    pub ascender: i32,
    pub descender: i32,
    pub line_gap: i32,
    pub source: VerticalMetricSource,
}

/// Positive distances in points, independent of glyph ink bounds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FontLineMetrics {
    pub ascent: f64,
    pub descent: f64,
    pub line_gap: f64,
}
impl FontLineMetrics {
    pub fn height(self) -> f64 {
        self.ascent + self.descent + self.line_gap
    }
    /// Include every participating face, including grapheme fallback faces.
    pub fn include(&mut self, other: Self) {
        self.ascent = self.ascent.max(other.ascent);
        self.descent = self.descent.max(other.descent);
        self.line_gap = self.line_gap.max(other.line_gap);
    }
}
impl FontVerticalMetrics {
    fn valid(self) -> bool {
        self.units_per_em != 0 && self.ascender > 0 && self.descender <= 0
    }
    pub fn at_size(self, size: f32) -> Result<FontLineMetrics, FontError> {
        if !self.valid() || !size.is_finite() || size <= 0.0 {
            return Err(FontError::InvalidRequest);
        }
        let scale = f64::from(size) / f64::from(self.units_per_em);
        Ok(FontLineMetrics {
            ascent: f64::from(self.ascender) * scale,
            descent: -f64::from(self.descender) * scale,
            // Negative leading must not make participating font boxes overlap.
            line_gap: f64::from(self.line_gap.max(0)) * scale,
        })
    }
}

pub(crate) fn read(face: &ttf_parser::Face<'_>) -> Result<FontVerticalMetrics, FontError> {
    let tables = face.tables();
    let os2 = tables.os2.ok_or(FontError::InvalidFont)?;
    let upem = face.units_per_em();
    let metrics = |source, a: i32, d: i32, gap: i32| FontVerticalMetrics {
        units_per_em: upem,
        ascender: a,
        descender: d,
        line_gap: gap,
        source,
    };
    let typo = metrics(
        VerticalMetricSource::Typographic,
        i32::from(os2.typographic_ascender()),
        i32::from(os2.typographic_descender()),
        i32::from(os2.typographic_line_gap()),
    );
    if os2.use_typographic_metrics() {
        // OS/2 version >= 4, fsSelection bit 7. Never mix table triplets.
        return if typo.valid() {
            Ok(typo)
        } else {
            Err(FontError::InvalidFont)
        };
    }
    let hhea = metrics(
        VerticalMetricSource::Hhea,
        i32::from(tables.hhea.ascender),
        i32::from(tables.hhea.descender),
        i32::from(tables.hhea.line_gap),
    );
    if hhea.valid() {
        return Ok(hhea);
    }
    if typo.valid() {
        return Ok(typo);
    }
    // usWin* are unsigned; the parser's signed convenience getters cannot
    // represent their full range. Read bounded bytes through its raw adapter.
    let raw = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"OS/2"))
        .ok_or(FontError::InvalidFont)?;
    let unsigned = |offset| -> Result<i32, FontError> {
        let bytes: [u8; 2] = raw
            .get(offset..offset + 2)
            .ok_or(FontError::InvalidFont)?
            .try_into()
            .map_err(|_| FontError::InvalidFont)?;
        Ok(i32::from(u16::from_be_bytes(bytes)))
    };
    let win = metrics(
        VerticalMetricSource::Windows,
        unsigned(74)?,
        -unsigned(76)?,
        0,
    );
    if win.valid() {
        Ok(win)
    } else {
        Err(FontError::InvalidFont)
    }
}
