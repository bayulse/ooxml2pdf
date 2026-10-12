use o2p_core::FontResource;
use std::{collections::BTreeMap, error::Error, fmt, fs::File, io::Read, path::Path, sync::Arc};

/// Hard ceilings also apply when fonts come from byte buffers or custom providers.
pub const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 16_000;
pub const MAX_GLYPHS: usize = 262_144;
const MAX_FONTS: usize = 256;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    InvalidFont,
    UnsupportedTechnology(&'static str),
    EmbeddingRestricted,
    LimitExceeded(&'static str),
    InvalidRequest,
    NoFonts,
    Io(String),
    Shaping(String),
}
impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "font error: {self:?}")
    }
}
impl Error for FontError {}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontWarning {
    MissingFont {
        family: String,
    },
    Substitution {
        requested: FontRequest,
        selected_family: String,
        selected_weight: u16,
        selected_italic: bool,
    },
    MissingGlyph {
        character: char,
    },
}
impl fmt::Display for FontWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontMetadata {
    pub family: String,
    pub postscript_name: String,
    pub weight: u16,
    pub italic: bool,
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub line_gap: i16,
    pub cap_height: i16,
    pub bounds: [i16; 4],
    pub italic_angle: f32,
    pub glyph_count: u16,
}

/// An immutable validated font; third-party parser/shaper types remain private.
pub struct FontFace {
    resource: Arc<FontResource>,
    metadata: FontMetadata,
    pub(crate) shaper: harfrust::Font,
}
impl fmt::Debug for FontFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontFace")
            .field("metadata", &self.metadata)
            .finish()
    }
}
impl FontFace {
    pub fn from_bytes(bytes: Arc<[u8]>) -> Result<Self, FontError> {
        Self::from_resource(Arc::new(FontResource { bytes }))
    }
    pub fn from_resource(resource: Arc<FontResource>) -> Result<Self, FontError> {
        let bytes = &resource.bytes;
        if bytes.len() > MAX_FONT_BYTES {
            return Err(FontError::LimitExceeded("font bytes"));
        }
        if bytes.starts_with(b"ttcf") {
            return Err(FontError::UnsupportedTechnology("font collection"));
        }
        let face = ttf_parser::Face::parse(bytes, 0).map_err(|_| FontError::InvalidFont)?;
        let raw = face.raw_face();
        for (tag, technology) in [
            (*b"CFF ", "CFF"),
            (*b"CFF2", "CFF2"),
            (*b"fvar", "variable font"),
            (*b"COLR", "color font"),
            (*b"CPAL", "color font"),
            (*b"SVG ", "SVG font"),
            (*b"CBDT", "bitmap font"),
            (*b"CBLC", "bitmap font"),
            (*b"EBDT", "bitmap font"),
            (*b"EBLC", "bitmap font"),
            (*b"bdat", "bitmap font"),
            (*b"bloc", "bitmap font"),
            (*b"sbix", "bitmap font"),
        ] {
            // Check records as well as parsed tables: malformed optional tables
            // must not accidentally turn unsupported fonts into supported ones.
            if raw
                .table_records
                .into_iter()
                .any(|r| r.tag == ttf_parser::Tag::from_bytes(&tag))
            {
                return Err(FontError::UnsupportedTechnology(technology));
            }
        }
        for tag in [
            b"glyf", b"loca", b"head", b"hhea", b"hmtx", b"maxp", b"cmap", b"name", b"OS/2",
        ] {
            if raw.table(ttf_parser::Tag::from_bytes(tag)).is_none() {
                return Err(FontError::InvalidFont);
            }
        }
        if face.number_of_glyphs() == 0 {
            return Err(FontError::InvalidFont);
        }
        if face.tables().glyf.is_none()
            || face.tables().hmtx.is_none()
            || face.tables().cmap.is_none()
            || face.tables().os2.is_none()
        {
            return Err(FontError::InvalidFont);
        }
        if matches!(
            face.permissions(),
            None | Some(ttf_parser::Permissions::Restricted)
        ) || !face.is_outline_embedding_allowed()
        {
            return Err(FontError::EmbeddingRestricted);
        }
        let name = |id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == id)
                .filter_map(|n| n.to_string())
                .find(|s| !s.is_empty() && s.len() <= 256)
        };
        let bounds = face.global_bounding_box();
        let metadata = FontMetadata {
            family: name(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
                .or_else(|| name(ttf_parser::name_id::FAMILY))
                .ok_or(FontError::InvalidFont)?,
            postscript_name: name(ttf_parser::name_id::POST_SCRIPT_NAME)
                .unwrap_or_else(|| "ConfiguredFont".into()),
            weight: face.weight().to_number(),
            italic: face.is_italic(),
            units_per_em: face.units_per_em(),
            ascent: face.ascender(),
            descent: face.descender(),
            line_gap: face.line_gap(),
            cap_height: face.capital_height().unwrap_or(face.ascender()),
            bounds: [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
            italic_angle: face.italic_angle(),
            glyph_count: face.number_of_glyphs(),
        };
        let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = resource.clone();
        let shaper = harfrust::Font::new(harfrust::font::Blob::from(shared), 0)
            .ok_or(FontError::InvalidFont)?;
        Ok(Self {
            resource,
            metadata,
            shaper,
        })
    }
    pub fn metadata(&self) -> &FontMetadata {
        &self.metadata
    }
    pub fn resource(&self) -> &Arc<FontResource> {
        &self.resource
    }
    pub fn glyph_id(&self, character: char) -> Option<u16> {
        ttf_parser::Face::parse(&self.resource.bytes, 0)
            .ok()?
            .glyph_index(character)
            .map(|g| g.0)
            .filter(|&id| id != 0)
    }
    pub fn glyph_advance(&self, id: u16) -> Option<u16> {
        ttf_parser::Face::parse(&self.resource.bytes, 0)
            .ok()?
            .glyph_hor_advance(ttf_parser::GlyphId(id))
    }
    pub fn supports(&self, text: &str) -> bool {
        text.chars()
            .all(|c| ignorable(c) || self.glyph_id(c).is_some())
    }
}
pub(crate) fn ignorable(c: char) -> bool {
    matches!(c, '\u{200c}' | '\u{200d}' | '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}

pub struct FontSelection {
    pub font: Arc<FontFace>,
    pub warnings: Vec<FontWarning>,
}

/// A consumer may implement this using bundled or platform-provided font data.
/// Resolution is at an extended grapheme boundary, never scalar-by-scalar.
pub trait FontResolver {
    fn resolve(&self, request: &FontRequest, cluster: &str) -> Result<FontSelection, FontError>;
}

/// Ordered explicit registry. No desktop font discovery or implicit bundled font.
#[derive(Debug, Default)]
pub struct FontBook {
    fonts: Vec<Arc<FontFace>>,
    aliases: BTreeMap<String, Vec<String>>,
    fallbacks: Vec<String>,
    total_bytes: usize,
}
impl FontBook {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn fonts(&self) -> &[Arc<FontFace>] {
        &self.fonts
    }
    pub fn register_bytes(&mut self, bytes: Arc<[u8]>) -> Result<Arc<FontFace>, FontError> {
        if self.fonts.len() >= MAX_FONTS {
            return Err(FontError::LimitExceeded("registered fonts"));
        }
        if bytes.len() > MAX_TOTAL_BYTES.saturating_sub(self.total_bytes) {
            return Err(FontError::LimitExceeded("total font bytes"));
        }
        let font = Arc::new(FontFace::from_bytes(bytes)?);
        self.total_bytes += font.resource.bytes.len();
        self.fonts.push(font.clone());
        Ok(font)
    }
    pub fn register_file(&mut self, path: impl AsRef<Path>) -> Result<Arc<FontFace>, FontError> {
        let file = File::open(path).map_err(|e| FontError::Io(e.to_string()))?;
        if file
            .metadata()
            .map_err(|e| FontError::Io(e.to_string()))?
            .len()
            > MAX_FONT_BYTES as u64
        {
            return Err(FontError::LimitExceeded("font bytes"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_FONT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| FontError::Io(e.to_string()))?;
        self.register_bytes(bytes.into())
    }
    /// Nonrecursive, sorted directory registration. Unsupported/invalid fonts
    /// return errors; successful earlier registrations remain in the registry.
    pub fn register_directory(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Vec<Arc<FontFace>>, FontError> {
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(path).map_err(|e| FontError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| FontError::Io(e.to_string()))?;
            if !entry
                .file_type()
                .map_err(|e| FontError::Io(e.to_string()))?
                .is_file()
            {
                continue;
            }
            let path = entry.path();
            if path.extension().is_some_and(|e| {
                e.eq_ignore_ascii_case("ttf")
                    || e.eq_ignore_ascii_case("otf")
                    || e.eq_ignore_ascii_case("ttc")
            }) {
                if paths.len() >= MAX_FONTS {
                    return Err(FontError::LimitExceeded("directory fonts"));
                }
                paths.push(path);
            }
        }
        paths.sort();
        paths.into_iter().map(|p| self.register_file(p)).collect()
    }
    pub fn set_fallback_families(&mut self, families: Vec<String>) {
        self.fallbacks = families;
    }
    pub fn set_family_alias(&mut self, requested: String, compatible: Vec<String>) {
        self.aliases.insert(key(&requested), compatible);
    }
}
fn key(s: &str) -> String {
    s.trim().to_lowercase()
}
impl FontResolver for FontBook {
    fn resolve(&self, request: &FontRequest, cluster: &str) -> Result<FontSelection, FontError> {
        if request.family.trim().is_empty()
            || request.family.len() > 256
            || !(1..=1000).contains(&request.weight)
        {
            return Err(FontError::InvalidRequest);
        }
        let requested = key(&request.family);
        let mut warnings = Vec::new();
        if requested != "sans-serif"
            && !self
                .fonts
                .iter()
                .any(|f| key(&f.metadata.family) == requested)
        {
            warnings.push(FontWarning::MissingFont {
                family: request.family.clone(),
            });
        }
        let mut families = vec![request.family.clone()];
        if let Some(aliases) = self.aliases.get(&requested) {
            families.extend(aliases.iter().cloned());
        }
        families.extend(self.fallbacks.iter().cloned());
        let mut first = None;
        let mut selected = None;
        for family in families {
            let mut candidates: Vec<_> = self
                .fonts
                .iter()
                .enumerate()
                .filter(|(_, f)| key(&f.metadata.family) == key(&family))
                .collect();
            candidates.sort_by_key(|(index, f)| {
                (
                    f.metadata.italic != request.italic,
                    f.metadata.weight.abs_diff(request.weight),
                    *index,
                )
            });
            for (_, font) in candidates {
                first.get_or_insert(font.clone());
                if font.supports(cluster) {
                    selected = Some(font.clone());
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
        let font = selected.or(first).ok_or(FontError::NoFonts)?;
        let m = font.metadata();
        if requested != key(&m.family) || request.weight != m.weight || request.italic != m.italic {
            warnings.push(FontWarning::Substitution {
                requested: request.clone(),
                selected_family: m.family.clone(),
                selected_weight: m.weight,
                selected_italic: m.italic,
            });
        }
        for c in cluster
            .chars()
            .filter(|&c| !ignorable(c) && font.glyph_id(c).is_none())
        {
            let warning = FontWarning::MissingGlyph { character: c };
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        Ok(FontSelection { font, warnings })
    }
}
