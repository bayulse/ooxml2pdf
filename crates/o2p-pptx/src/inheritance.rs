//! Owned semantic inheritance; no XML nodes or package implementation types.
use crate::{Fill, PptxError, PptxWarning, Shape, TextAlignment, TextStyle};
use o2p_core::{Color, Point, Rect, Size, Transform};
use o2p_ooxml::{OoxmlPackage, RelationshipTarget};

/// Sparse geometry in points. Missing components inherit independently.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceholderGeometry {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub rotation: Option<f32>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
}
impl PlaceholderGeometry {
    pub(crate) fn inherit(&mut self, parent: &Self) {
        self.x = self.x.or(parent.x);
        self.y = self.y.or(parent.y);
        self.width = self.width.or(parent.width);
        self.height = self.height.or(parent.height);
        self.rotation = self.rotation.or(parent.rotation);
        self.flip_h = self.flip_h.or(parent.flip_h);
        self.flip_v = self.flip_v.or(parent.flip_v);
    }
    pub(crate) fn resolved(&self) -> Option<(Rect, Transform)> {
        let rect = Rect {
            origin: Point {
                x: self.x?,
                y: self.y?,
            },
            size: Size {
                width: self.width?,
                height: self.height?,
            },
        };
        let center = Point {
            x: rect.origin.x + rect.size.width / 2.0,
            y: rect.origin.y + rect.size.height / 2.0,
        };
        Some((
            rect,
            Transform::rotation_and_flips(
                self.rotation.unwrap_or(0.0),
                self.flip_h.unwrap_or(false),
                self.flip_v.unwrap_or(false),
            )
            .around(center),
        ))
    }
}

/// Supported sparse text defaults. A present unresolved family/color blocks inheritance.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceholderTextDefaults {
    pub family: Option<Option<String>>,
    pub font_size: Option<f32>,
    pub color: Option<Option<Color>>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub alignment: Option<TextAlignment>,
}
impl PlaceholderTextDefaults {
    pub(crate) fn inherit(&mut self, parent: &Self) {
        self.family = self.family.clone().or_else(|| parent.family.clone());
        self.font_size = self.font_size.or(parent.font_size);
        self.color = self.color.or(parent.color);
        self.bold = self.bold.or(parent.bold);
        self.italic = self.italic.or(parent.italic);
        self.alignment = self.alignment.or(parent.alignment);
    }
    pub(crate) fn style(&self) -> TextStyle {
        let mut style = TextStyle::default();
        style.family = self.family.clone().flatten();
        style.font_size = self.font_size.unwrap_or(style.font_size);
        style.color = self.color.flatten().unwrap_or(style.color);
        style.bold = self.bold.unwrap_or(style.bold);
        style.italic = self.italic.unwrap_or(style.italic);
        style
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Properties {
    pub geometry: PlaceholderGeometry,
    pub geometry_blocked: bool,
    pub preset: Option<String>,
    pub fill: Option<Option<Fill>>,
    pub line_width: Option<f32>,
    pub line_color: Option<Option<Color>>,
    pub text: PlaceholderTextDefaults,
}
impl Properties {
    pub(crate) fn inherit(&mut self, parent: &Self) {
        if parent.geometry_blocked {
            // A complete local transform/preset can replace an unsupported ancestor.
            self.geometry_blocked |= self.geometry.resolved().is_none() || self.preset.is_none();
        } else {
            self.geometry.inherit(&parent.geometry);
        }
        self.preset = self.preset.clone().or_else(|| parent.preset.clone());
        self.fill = self.fill.or(parent.fill);
        self.line_width = self.line_width.or(parent.line_width);
        self.line_color = self.line_color.or(parent.line_color);
        self.text.inherit(&parent.text);
    }
}

/// Placeholder identity is independent of visible text and drawing coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Placeholder {
    pub(crate) kind: String,
    pub(crate) idx: Option<u32>,
    pub(crate) properties: Properties,
}
impl Placeholder {
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn idx(&self) -> Option<u32> {
        self.idx
    }
    pub fn geometry(&self) -> &PlaceholderGeometry {
        &self.properties.geometry
    }
    pub fn text_defaults(&self) -> &PlaceholderTextDefaults {
        &self.properties.text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideLayout {
    pub(crate) part: String,
    pub(crate) master_part: Option<String>,
    pub(crate) source: Source,
    pub(crate) theme: Option<crate::Theme>,
}
impl SlideLayout {
    pub fn part_name(&self) -> &str {
        &self.part
    }
    pub fn master_part_name(&self) -> Option<&str> {
        self.master_part.as_deref()
    }
    pub fn placeholders(&self) -> &[Placeholder] {
        &self.source.placeholders
    }
    pub fn decorative_shapes(&self) -> &[Shape] {
        &self.source.slide.shapes
    }
    pub fn background(&self) -> Option<Fill> {
        self.source.slide.background
    }
    pub fn theme(&self) -> Option<&crate::Theme> {
        self.theme.as_ref()
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SlideMaster {
    pub(crate) part: String,
    pub(crate) source: Source,
    pub(crate) theme: Option<crate::Theme>,
}
impl SlideMaster {
    pub fn part_name(&self) -> &str {
        &self.part
    }
    pub fn placeholders(&self) -> &[Placeholder] {
        &self.source.placeholders
    }
    pub fn decorative_shapes(&self) -> &[Shape] {
        &self.source.slide.shapes
    }
    pub fn background(&self) -> Option<Fill> {
        self.source.slide.background
    }
    pub fn theme(&self) -> Option<&crate::Theme> {
        self.theme.as_ref()
    }
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Source {
    pub slide: crate::Slide,
    pub placeholders: Vec<Placeholder>,
    pub background_present: bool,
    pub show_master_shapes: bool,
}

pub(crate) fn relationship_kind(uri: &str, kind: &str) -> bool {
    [
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/",
        "http://purl.oclc.org/ooxml/officeDocument/relationships/",
    ]
    .iter()
    .any(|prefix| uri.strip_prefix(prefix) == Some(kind))
}
pub(crate) fn related(
    package: &OoxmlPackage,
    source: &str,
    kind: &str,
    warnings: &mut Vec<PptxWarning>,
    slide_id: u32,
) -> Result<Option<String>, PptxError> {
    let mut matches = package
        .relationships(Some(source))?
        .iter()
        .filter(|r| relationship_kind(&r.relationship_type, kind));
    let Some(relation) = matches.next() else {
        warnings.push(PptxWarning::UnsupportedSlideContent {
            slide_id,
            part: source.into(),
            name: format!("missing {kind} relationship: inheritance unavailable"),
        });
        return Ok(None);
    };
    let invalid = |message: &str| PptxError::InvalidSlideRelationship {
        part: source.into(),
        relationship_id: relation.id.clone(),
        message: message.into(),
    };
    if matches.next().is_some() {
        return Err(invalid("multiple inheritance relationships"));
    }
    let RelationshipTarget::Internal(target) = &relation.target else {
        return Err(invalid("inheritance target must be internal"));
    };
    let expected =
        format!("application/vnd.openxmlformats-officedocument.presentationml.{kind}+xml");
    if package.part(target)?.content_type() != expected {
        return Err(invalid("inheritance target has wrong content type"));
    }
    Ok(Some(target.clone()))
}

/// Slide->layout uses idx (default zero); layout->master uses compatible type.
/// Only unique candidates are safe. Explicit unmatched slide idx never falls back.
pub(crate) fn matching<'a>(
    child: &Placeholder,
    parents: &'a [Placeholder],
    master: bool,
) -> Result<Option<&'a Placeholder>, ()> {
    fn master_kind(kind: &str) -> &str {
        match kind {
            "ctrTitle" => "title",
            "subTitle" | "obj" | "object" | "text" => "body",
            other => other,
        }
    }
    let candidates: Vec<_> = if master {
        parents
            .iter()
            .filter(|p| master_kind(&p.kind) == master_kind(&child.kind))
            .collect()
    } else {
        parents
            .iter()
            .filter(|p| p.idx.unwrap_or(0) == child.idx.unwrap_or(0))
            .collect()
    };
    if candidates.len() == 1 {
        return Ok(Some(candidates[0]));
    }
    if candidates.len() > 1 {
        return Err(());
    }
    if !master && child.idx.is_none() {
        let mut same_type = parents.iter().filter(|p| p.kind == child.kind);
        let first = same_type.next();
        if same_type.next().is_some() {
            return Err(());
        }
        return Ok(first);
    }
    Ok(None)
}

/// Bounded, fixed-depth traversal. Shared ancestors and raster assets are parsed once.
pub(crate) struct Reader<'a> {
    package: &'a OoxmlPackage,
    limits: crate::ParseLimits,
    warnings: &'a mut Vec<PptxWarning>,
    fallback_theme: Option<&'a crate::Theme>,
    images: std::collections::BTreeMap<String, std::sync::Arc<o2p_core::ImageResource>>,
    pub layouts: std::collections::BTreeMap<String, SlideLayout>,
    pub masters: std::collections::BTreeMap<String, SlideMaster>,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(
        package: &'a OoxmlPackage,
        limits: crate::ParseLimits,
        warnings: &'a mut Vec<PptxWarning>,
        fallback_theme: Option<&'a crate::Theme>,
    ) -> Self {
        Self {
            package,
            limits,
            warnings,
            fallback_theme,
            images: Default::default(),
            layouts: Default::default(),
            masters: Default::default(),
        }
    }
    fn source(
        &mut self,
        reference: &crate::SlideRef,
        root: &str,
        parents: &[Placeholder],
        theme: Option<&crate::Theme>,
    ) -> Result<Source, PptxError> {
        let source = crate::slide::parse(
            self.package,
            &mut self.images,
            self.package.part(reference.part_name())?.bytes(),
            reference,
            self.limits,
            self.warnings,
            crate::slide::ParseSource {
                root,
                parents,
                master_matching: root == "sldLayout",
                theme,
            },
        )?;
        for relation in self.package.relationships(Some(reference.part_name()))? {
            if relationship_kind(&relation.relationship_type, "image")
                || (root != "sld" && relationship_kind(&relation.relationship_type, "theme"))
                || (root != "sldLayout"
                    && relationship_kind(&relation.relationship_type, "slideLayout"))
                || (root == "sldLayout"
                    && relationship_kind(&relation.relationship_type, "slideMaster"))
            {
                continue;
            }
            self.warnings.push(PptxWarning::UnsupportedSlideContent {
                slide_id: reference.id(),
                part: reference.part_name().into(),
                name: format!(
                    "{root} relationship {}",
                    crate::xml::expanded(None, &relation.relationship_type)
                ),
            });
        }
        Ok(source)
    }
    pub(crate) fn slide(&mut self, reference: &crate::SlideRef) -> Result<crate::Slide, PptxError> {
        let mut missing_link_warnings = vec![];
        let layout_part = related(
            self.package,
            reference.part_name(),
            "slideLayout",
            &mut missing_link_warnings,
            reference.id(),
        )?;
        if let Some(part) = &layout_part {
            if !self.layouts.contains_key(part) {
                let master_part = related(
                    self.package,
                    part,
                    "slideMaster",
                    self.warnings,
                    reference.id(),
                )?;
                if let Some(master) = &master_part {
                    if !self.masters.contains_key(master) {
                        let theme = crate::theme_xml::load(
                            self.package,
                            master,
                            self.limits,
                            self.warnings,
                        )?
                        .or_else(|| self.fallback_theme.cloned());
                        let master_ref = crate::SlideRef {
                            id: reference.id(),
                            part_name: master.clone(),
                        };
                        let source = self.source(&master_ref, "sldMaster", &[], theme.as_ref())?;
                        self.masters.insert(
                            master.clone(),
                            SlideMaster {
                                part: master.clone(),
                                source,
                                theme,
                            },
                        );
                    }
                }
                let master = master_part.as_ref().and_then(|p| self.masters.get(p));
                let parents = master
                    .map(|m| m.source.placeholders.clone())
                    .unwrap_or_default();
                let theme = crate::theme_xml::load(self.package, part, self.limits, self.warnings)?
                    .or_else(|| master.and_then(|m| m.theme.clone()))
                    .or_else(|| self.fallback_theme.cloned());
                let layout_ref = crate::SlideRef {
                    id: reference.id(),
                    part_name: part.clone(),
                };
                let source = self.source(&layout_ref, "sldLayout", &parents, theme.as_ref())?;
                self.layouts.insert(
                    part.clone(),
                    SlideLayout {
                        part: part.clone(),
                        master_part,
                        source,
                        theme,
                    },
                );
            }
        }
        let layout = layout_part.as_ref().and_then(|p| self.layouts.get(p));
        let parents = layout
            .map(|l| l.source.placeholders.clone())
            .unwrap_or_default();
        let theme = layout
            .and_then(|l| l.theme.clone())
            .or_else(|| self.fallback_theme.cloned());
        let mut local = self.source(reference, "sld", &parents, theme.as_ref())?;
        self.warnings.extend(missing_link_warnings);
        let layout = layout_part.as_ref().and_then(|p| self.layouts.get(p));
        let master = layout
            .and_then(|l| l.master_part.as_ref())
            .and_then(|p| self.masters.get(p));
        if !local.background_present {
            if let Some(layout) = layout.filter(|l| l.source.background_present) {
                local.slide.background = layout.source.slide.background;
            } else if let Some(master) = master {
                local.slide.background = master.source.slide.background;
            }
        }
        let mut shapes = vec![];
        if local.show_master_shapes && layout.is_none_or(|l| l.source.show_master_shapes) {
            if let Some(master) = master {
                shapes.extend(master.source.slide.shapes.iter().cloned());
            }
        }
        if let Some(layout) = layout {
            shapes.extend(layout.source.slide.shapes.iter().cloned());
        }
        shapes.append(&mut local.slide.shapes);
        local.slide.shapes = shapes;
        local.slide.placeholders = local.placeholders;
        Ok(local.slide)
    }
}
