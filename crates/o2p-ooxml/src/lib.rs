//! Bounded, offline OPC package access. No Office content is rendered or parsed.
//!
//! ZIP and metadata XML adapters are private. Parts remain in memory after open;
//! callers configure resource limits before accepting untrusted input.

mod paths;
mod xml;
mod zip;

use std::{collections::BTreeMap, fmt, fs::File, io::Read, path::Path};

pub use paths::{normalize_part_path, resolve_internal_target};

/// Limits apply before allocation/decompression and again to actual output bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityLimits {
    pub max_input_size: u64,
    pub max_zip_entries: usize,
    pub max_total_uncompressed_size: u64,
    pub max_part_size: u64,
    pub max_compression_ratio: u64,
    pub max_xml_size: u64,
    pub max_relationships: usize,
    /// Maximum bytes in a ZIP name or resolved internal part name.
    pub max_path_length: usize,
}

impl Default for SecurityLimits {
    fn default() -> Self {
        Self {
            max_input_size: 64 * 1024 * 1024,
            max_zip_entries: 10_000,
            max_total_uncompressed_size: 256 * 1024 * 1024,
            max_part_size: 64 * 1024 * 1024,
            max_compression_ratio: 1_000,
            max_xml_size: 4 * 1024 * 1024,
            max_relationships: 100_000,
            max_path_length: 1_024,
        }
    }
}

/// Library-owned failures; no concrete parser or decompressor errors escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    Io(String),
    InvalidZip(String),
    InvalidXml { part: String, message: String },
    InvalidPath(String),
    InvalidMetadata(String),
    MissingPart(String),
    DuplicatePart(String),
    LimitExceeded { limit: &'static str },
    Unsupported(String),
    NotPptx,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "Package I/O failed: {message}"),
            Self::InvalidZip(message) => write!(f, "Invalid ZIP: {message}"),
            Self::InvalidXml { part, message } => write!(f, "Invalid XML in {part}: {message}"),
            Self::InvalidPath(path) => write!(f, "Invalid package path: {path}"),
            Self::InvalidMetadata(message) => write!(f, "Invalid OPC metadata: {message}"),
            Self::MissingPart(path) => write!(f, "Missing package part: {path}"),
            Self::DuplicatePart(path) => write!(f, "Duplicate package part: {path}"),
            Self::LimitExceeded { limit } => write!(f, "Package security limit exceeded: {limit}"),
            Self::Unsupported(message) => write!(f, "Unsupported package feature: {message}"),
            Self::NotPptx => write!(f, "Package is not a PPTX presentation"),
        }
    }
}
impl std::error::Error for PackageError {}

type Result<T> = std::result::Result<T, PackageError>;

fn limit(limit: &'static str) -> PackageError {
    PackageError::LimitExceeded { limit }
}

/// Content type declarations, with overrides taking precedence over extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Default {
        extension: String,
        mime_type: String,
    },
    Override {
        part_name: String,
        mime_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipTarget {
    /// Canonical package-root-relative part name. Never a filesystem path.
    Internal(String),
    /// Opaque external URI. The reader never opens or fetches this target.
    External(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub relationship_type: String,
    pub target: RelationshipTarget,
}

/// Immutable part bytes. All fields are accessible through getters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePart {
    name: String,
    content_type: String,
    bytes: Vec<u8>,
}
impl PackagePart {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn content_type(&self) -> &str {
        &self.content_type
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Package inventories, sorted by canonical name, not presentation playback order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PptxInspection {
    pub presentation_part: String,
    pub slides: Vec<String>,
    pub media: Vec<String>,
}
impl PptxInspection {
    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
    pub fn media_count(&self) -> usize {
        self.media.len()
    }
}

/// Validated package. No ZIP extraction, network access or third-party public types.
#[derive(Debug)]
pub struct OoxmlPackage {
    parts: BTreeMap<String, PackagePart>,
    content_types: Vec<ContentType>,
    relationships: BTreeMap<String, Vec<Relationship>>,
}

impl OoxmlPackage {
    pub fn open(path: impl AsRef<Path>, limits: SecurityLimits) -> Result<Self> {
        let file = File::open(path).map_err(|e| PackageError::Io(e.to_string()))?;
        if file
            .metadata()
            .map_err(|e| PackageError::Io(e.to_string()))?
            .len()
            > limits.max_input_size
        {
            return Err(limit("max_input_size"));
        }
        let mut bytes = Vec::new();
        file.take(limits.max_input_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| PackageError::Io(e.to_string()))?;
        Self::from_bytes(&bytes, limits)
    }

    pub fn from_bytes(bytes: &[u8], limits: SecurityLimits) -> Result<Self> {
        let mut raw = zip::read(bytes, limits)?;
        let metadata = raw
            .remove("[Content_Types].xml")
            .ok_or_else(|| PackageError::MissingPart("[Content_Types].xml".into()))?;
        let content_types = xml::content_types(&metadata, limits)?;
        let mut defaults = BTreeMap::new();
        let mut overrides = BTreeMap::new();
        for declaration in &content_types {
            match declaration {
                ContentType::Default {
                    extension,
                    mime_type,
                } => {
                    defaults.insert(extension.to_ascii_lowercase(), mime_type.clone());
                }
                ContentType::Override {
                    part_name,
                    mime_type,
                } => {
                    if !raw.contains_key(part_name) {
                        return Err(PackageError::MissingPart(part_name.clone()));
                    }
                    overrides.insert(part_name.clone(), mime_type.clone());
                }
            }
        }
        let mut parts = BTreeMap::new();
        for (name, bytes) in raw {
            let mime = overrides
                .get(&name)
                .or_else(|| {
                    name.rsplit_once('.')
                        .and_then(|(_, extension)| defaults.get(&extension.to_ascii_lowercase()))
                })
                .ok_or_else(|| {
                    PackageError::InvalidMetadata(format!("No content type for {name}"))
                })?;
            parts.insert(
                name.clone(),
                PackagePart {
                    name,
                    content_type: mime.clone(),
                    bytes,
                },
            );
        }
        if !parts.contains_key("_rels/.rels") {
            return Err(PackageError::MissingPart("_rels/.rels".into()));
        }
        let mut relationships = BTreeMap::new();
        let mut relationship_count = 0usize;
        for part in parts.values() {
            if let Some(source) = paths::relationship_source(&part.name)? {
                if part.content_type != "application/vnd.openxmlformats-package.relationships+xml" {
                    return Err(PackageError::InvalidMetadata(format!(
                        "Invalid relationship content type for {}",
                        part.name
                    )));
                }
                if !source.is_empty() && !parts.contains_key(&source) {
                    return Err(PackageError::MissingPart(source));
                }
                let parsed = xml::relationships(&part.bytes, &part.name, &source, limits)?;
                relationship_count = relationship_count
                    .checked_add(parsed.len())
                    .ok_or_else(|| limit("max_relationships"))?;
                if relationship_count > limits.max_relationships {
                    return Err(limit("max_relationships"));
                }
                for relation in &parsed {
                    if let RelationshipTarget::Internal(target) = &relation.target {
                        if !parts.contains_key(target) {
                            return Err(PackageError::MissingPart(target.clone()));
                        }
                    }
                }
                relationships.insert(source, parsed);
            } else if part.content_type
                == "application/vnd.openxmlformats-package.relationships+xml"
            {
                return Err(PackageError::InvalidMetadata(format!(
                    "Invalid relationship part name {}",
                    part.name
                )));
            }
        }
        Ok(Self {
            parts,
            content_types,
            relationships,
        })
    }

    /// The content-types stream is metadata, rather than an ordinary package part.
    pub fn content_types(&self) -> &[ContentType] {
        &self.content_types
    }
    pub fn parts(&self) -> impl Iterator<Item = &PackagePart> {
        self.parts.values()
    }
    pub fn part(&self, name: &str) -> Result<&PackagePart> {
        let name = normalize_part_path(name)?;
        self.parts.get(&name).ok_or(PackageError::MissingPart(name))
    }
    /// `None` selects package-level relationships; parts without `.rels` return `[]`.
    pub fn relationships(&self, source: Option<&str>) -> Result<&[Relationship]> {
        let source = match source {
            Some(name) => self.part(name)?.name.clone(),
            None => String::new(),
        };
        Ok(self
            .relationships
            .get(&source)
            .map(Vec::as_slice)
            .unwrap_or(&[]))
    }
    pub fn is_pptx(&self) -> bool {
        self.presentation().is_some()
    }

    fn presentation(&self) -> Option<&PackagePart> {
        let roots = self.relationships.get("")?;
        let mut documents = roots.iter().filter(|r| {
            matches!(
                r.relationship_type.as_str(),
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
                    | "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument"
            )
        });
        let relation = documents.next()?;
        if documents.next().is_some() {
            return None;
        }
        let RelationshipTarget::Internal(name) = &relation.target else {
            return None;
        };
        self.parts.get(name).filter(|part| part.content_type ==
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml")
    }

    /// Inspect package metadata only. Slide contents and presentation order are deferred.
    pub fn inspect_pptx(&self) -> Result<PptxInspection> {
        let presentation = self.presentation().ok_or(PackageError::NotPptx)?;
        const SLIDE: &str =
            "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
        let slides = self
            .parts
            .values()
            .filter(|p| p.content_type == SLIDE)
            .map(|p| p.name.clone())
            .collect();
        let directory = presentation
            .name
            .rsplit_once('/')
            .map_or("", |(dir, _)| dir);
        let media_prefix = if directory.is_empty() {
            "media/".into()
        } else {
            format!("{directory}/media/")
        };
        let media = self
            .parts
            .values()
            .filter(|p| {
                p.name.starts_with(&media_prefix)
                    && p.content_type != "application/vnd.openxmlformats-package.relationships+xml"
            })
            .map(|p| p.name.clone())
            .collect();
        Ok(PptxInspection {
            presentation_part: presentation.name.clone(),
            slides,
            media,
        })
    }
}
