//! PresentationML -> normalized presentation and slides -> common DisplayList.
//!
//! Metadata-only parsing keeps slides opaque; content parsing supports solid
//! backgrounds, rectangles, ellipses, straight lines, embedded raster pictures and basic text.
//! No PDF commands or external loading occur.

mod font_layout;
mod inheritance;
mod model;
pub use font_layout::{FontDisplayReport, TextLayoutWarning, TextLayoutWarningKind};
pub use inheritance::{
    Placeholder, PlaceholderGeometry, PlaceholderTextDefaults, SlideLayout, SlideMaster,
};
mod slide;
mod theme;
mod theme_xml;
mod xml;
pub use theme::{ColorScheme, FontScheme, Theme, ThemeColorSlot, ThemeFonts};

pub use model::{
    Fill, Paragraph, Shape, Slide, Stroke, TextAlignment, TextBox, TextInsets, TextRun, TextStyle,
};

use std::{collections::BTreeMap, fmt};

use o2p_core::{DisplayList, Page, Size};
use o2p_ooxml::{OoxmlPackage, PackageError, RelationshipTarget};

const SLIDE_TYPE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const SLIDE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const STRICT_SLIDE_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships/slide";

/// Independent limits for each presentation/slide XML part and the slide list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub max_xml_size: usize,
    pub max_xml_nodes: u32,
    /// May lower, but not raise, the adapter's hard safety ceiling of 64 elements.
    pub max_xml_depth: usize,
    pub max_slides: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_xml_size: 4 * 1024 * 1024,
            max_xml_nodes: 100_000,
            max_xml_depth: 64,
            max_slides: 10_000,
        }
    }
}

/// Owned errors. XML implementation types never cross the public boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PptxError {
    /// A discovered theme contains invalid supported semantics or relationships.
    InvalidTheme {
        part: String,
        message: String,
    },
    Package(PackageError),
    InvalidXml {
        part: String,
        message: String,
    },
    InvalidMetadata {
        part: String,
        message: String,
    },
    InvalidSlide {
        part: String,
        message: String,
    },
    InvalidSlideRelationship {
        part: String,
        relationship_id: String,
        message: String,
    },
    LimitExceeded {
        limit: &'static str,
    },
}

impl fmt::Display for PptxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTheme { part, message } => write!(f, "Invalid theme in {part}: {message}"),
            Self::Package(error) => write!(f, "{error}"),
            Self::InvalidXml { part, message } => write!(f, "Invalid XML in {part}: {message}"),
            Self::InvalidMetadata { part, message } => {
                write!(f, "Invalid presentation metadata in {part}: {message}")
            }
            Self::InvalidSlide { part, message } => {
                write!(f, "Invalid slide in {part}: {message}")
            }
            Self::InvalidSlideRelationship {
                part,
                relationship_id,
                message,
            } => {
                write!(
                    f,
                    "Invalid slide relationship {relationship_id} in {part}: {message}"
                )
            }
            Self::LimitExceeded { limit } => write!(f, "PPTX security limit exceeded: {limit}"),
        }
    }
}

impl std::error::Error for PptxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Package(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PackageError> for PptxError {
    fn from(error: PackageError) -> Self {
        Self::Package(error)
    }
}

/// Unsupported metadata and slide features in the implemented parsing subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PptxWarning {
    /// A theme resource or definition is outside the supported semantic subset.
    UnsupportedTheme { part: String, name: String },
    /// Metadata-only parsing warns for every reference, including empty slides.
    SlideContentNotRendered { slide_id: u32, part: String },
    /// An element or attribute outside the supported metadata subset is opaque.
    UnsupportedPresentationMetadata { part: String, name: String },
    /// A slide element, attribute, color, or inherited property is unsupported.
    UnsupportedSlideContent {
        slide_id: u32,
        part: String,
        name: String,
    },
}

impl fmt::Display for PptxWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTheme { part, name } => write!(f, "{part}: unsupported theme {name}"),
            Self::SlideContentNotRendered { slide_id, part } => {
                write!(
                    f,
                    "Slide {slide_id} ({part}): content is not rendered; page is blank"
                )
            }
            Self::UnsupportedPresentationMetadata { part, name } => {
                write!(
                    f,
                    "{part}: unsupported presentation metadata {name} is not interpreted"
                )
            }
            Self::UnsupportedSlideContent {
                slide_id,
                part,
                name,
            } => {
                write!(f, "Slide {slide_id} ({part}): unsupported {name}")
            }
        }
    }
}

/// Global dimensions in EMUs, preserved exactly until conversion to core points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideSize {
    width_emu: u32,
    height_emu: u32,
}

impl SlideSize {
    pub fn width_emu(self) -> u32 {
        self.width_emu
    }

    pub fn height_emu(self) -> u32 {
        self.height_emu
    }

    pub fn to_points(self) -> Size {
        Size {
            width: emu_to_points(self.width_emu),
            height: emu_to_points(self.height_emu),
        }
    }
}

/// Convert EMUs to core points: 914,400 EMUs/inch / 72 points/inch = 12,700.
pub fn emu_to_points(emu: impl Into<i64>) -> f32 {
    // Divide in f64 before rounding to the core's f32 representation.
    (emu.into() as f64 / 12_700.0) as f32
}

/// One reference in presentation order; hidden slides also produce a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideRef {
    id: u32,
    part_name: String,
}

impl SlideRef {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn part_name(&self) -> &str {
        &self.part_name
    }
}

/// Owned metadata and optional slide content, independent of package/XML lifetimes.
#[derive(Debug, Clone, PartialEq)]
pub struct Presentation {
    size: SlideSize,
    slides: Vec<SlideRef>,
    warnings: Vec<PptxWarning>,
    contents: Vec<Slide>,
    theme: Option<Theme>,
    layouts: Vec<SlideLayout>,
    masters: Vec<SlideMaster>,
}

impl Presentation {
    /// Read the package's detected PPTX main part (normally ppt/presentation.xml).
    ///
    /// Requires a positive slide size and at least one slide reference. Orphan
    /// slide parts are not pages. Both Transitional and Strict namespaces work.
    pub fn parse(package: &OoxmlPackage, limits: ParseLimits) -> Result<Self, PptxError> {
        let main = package.inspect_pptx()?.presentation_part;
        let part = package.part(&main)?;
        let metadata = xml::parse(part.bytes(), &main, limits)?;
        let relationships: BTreeMap<_, _> = package
            .relationships(Some(&main))?
            .iter()
            .map(|relationship| (relationship.id.as_str(), relationship))
            .collect();
        let mut slides = Vec::with_capacity(metadata.slides.len());
        let mut warnings = metadata.warnings;
        for (id, relationship_id) in metadata.slides {
            let invalid = |message: &str| PptxError::InvalidSlideRelationship {
                part: main.clone(),
                relationship_id: relationship_id.clone(),
                message: message.into(),
            };
            let relationship = relationships
                .get(relationship_id.as_str())
                .ok_or_else(|| invalid("missing relationship"))?;
            if !matches!(
                relationship.relationship_type.as_str(),
                SLIDE_REL | STRICT_SLIDE_REL
            ) {
                return Err(invalid("relationship type is not slide"));
            }
            let RelationshipTarget::Internal(target) = &relationship.target else {
                return Err(invalid("slide target must be internal"));
            };
            if package.part(target)?.content_type() != SLIDE_TYPE {
                return Err(invalid("target content type is not PresentationML slide"));
            }
            // No slide bytes are read or parsed here.
            slides.push(SlideRef {
                id,
                part_name: target.clone(),
            });
            warnings.push(PptxWarning::SlideContentNotRendered {
                slide_id: id,
                part: target.clone(),
            });
        }
        Ok(Self {
            size: metadata.size,
            slides,
            warnings,
            contents: vec![],
            theme: None,
            layouts: vec![],
            masters: vec![],
        })
    }

    /// Parse referenced slide parts in presentation order as well as metadata.
    /// Byte, node and depth limits apply independently to each XML part.
    pub fn parse_with_content(
        package: &OoxmlPackage,
        limits: ParseLimits,
    ) -> Result<Self, PptxError> {
        let mut presentation = Self::parse(package, limits)?;
        let main = package.inspect_pptx()?.presentation_part;
        presentation.theme = theme_xml::load(package, &main, limits, &mut presentation.warnings)?;
        presentation
            .warnings
            .retain(|warning| !matches!(warning, PptxWarning::SlideContentNotRendered { .. }));
        let mut reader = inheritance::Reader::new(
            package,
            limits,
            &mut presentation.warnings,
            presentation.theme.as_ref(),
        );
        for reference in &presentation.slides {
            presentation.contents.push(reader.slide(reference)?);
        }
        presentation.layouts = reader.layouts.into_values().collect();
        presentation.masters = reader.masters.into_values().collect();
        Ok(presentation)
    }

    /// Parsed content in `slides()` order; empty after metadata-only parsing.
    pub fn slide_contents(&self) -> &[Slide] {
        &self.contents
    }

    /// The presentation's explicit theme resource; metadata-only parsing stays opaque.
    pub fn theme(&self) -> Option<&Theme> {
        self.theme.as_ref()
    }
    /// Reachable parsed layouts, sorted by canonical part name.
    pub fn slide_layouts(&self) -> &[SlideLayout] {
        &self.layouts
    }
    /// Reachable parsed masters, sorted by canonical part name.
    pub fn slide_masters(&self) -> &[SlideMaster] {
        &self.masters
    }

    /// Paint effective backgrounds, master decorations, layout decorations,
    /// then slide shapes; retain XML order within each layer. Metadata-only models
    /// produce blank pages and retain their existing unrendered-content warnings.
    pub fn to_display_list(&self) -> DisplayList {
        if self.contents.is_empty() {
            return self.to_blank_display_list();
        }
        DisplayList {
            pages: self
                .contents
                .iter()
                .map(|slide| slide.to_page(self.size.to_points()))
                .collect(),
        }
    }

    pub fn size(&self) -> SlideSize {
        self.size
    }

    pub fn slides(&self) -> &[SlideRef] {
        &self.slides
    }

    pub fn warnings(&self) -> &[PptxWarning] {
        &self.warnings
    }

    /// Emit one blank page per ordered reference, using the real global size.
    /// Page index corresponds exactly to `slides()[index]`.
    pub fn to_blank_display_list(&self) -> DisplayList {
        let size = self.size.to_points();
        DisplayList {
            pages: self
                .slides
                .iter()
                .map(|_| Page { size, ops: vec![] })
                .collect(),
        }
    }
}

/// DrawingML ST_Angle uses signed 1/60,000 degrees. Normalize whole turns
/// in integer units before conversion, preserving negative rotations exactly.
pub fn drawingml_rotation_to_degrees(units: i32) -> f32 {
    (i64::from(units).rem_euclid(360 * 60_000) as f64 / 60_000.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emu_conversion_uses_points_and_rounds_only_after_division() {
        assert_eq!(emu_to_points(0), 0.0);
        assert_eq!(emu_to_points(12_700), 1.0);
        assert_eq!(emu_to_points(914_400), 72.0);
        assert_eq!(emu_to_points(9_144_000), 720.0);
        assert_eq!(emu_to_points(12_192_000), 960.0);
        assert_eq!(emu_to_points(6_858_000), 540.0);
        assert!((emu_to_points(1) - 1.0 / 12_700.0).abs() < f32::EPSILON);
        assert!(emu_to_points(u32::MAX).is_finite());
    }
}
