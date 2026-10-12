//! PresentationML/DrawingML semantics, isolated from OPC and PDF rendering.
#[path = "text.rs"]
mod text;
use o2p_core::{Color, ImageFormat, ImageOp, ImageResource, Point, Rect, Size};
use o2p_ooxml::{OoxmlPackage, RelationshipTarget};
use roxmltree::Node;
use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Fill, Paragraph, ParseLimits, PptxError, PptxWarning, Shape, Slide, SlideRef, Stroke,
    TextAlignment, TextBox, TextInsets, TextRun, TextStyle, emu_to_points, xml,
};

const PML: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const STRICT_PML: &str = "http://purl.oclc.org/ooxml/presentationml/main";
const DML: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const STRICT_DML: &str = "http://purl.oclc.org/ooxml/drawingml/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const STRICT_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";

pub(super) fn is_image_relationship(kind: &str) -> bool {
    matches!(
        kind,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
            | "http://purl.oclc.org/ooxml/officeDocument/relationships/image"
    )
}

pub(super) fn parse(
    package: &OoxmlPackage,
    images: &mut BTreeMap<String, Arc<ImageResource>>,
    bytes: &[u8],
    reference: &SlideRef,
    limits: ParseLimits,
    warnings: &mut Vec<PptxWarning>,
) -> Result<Slide, PptxError> {
    xml::with_document(bytes, reference.part_name(), limits, |document| {
        let root = document.root_element();
        let pml = root.tag_name().namespace().unwrap_or("");
        if root.tag_name().name() != "sld" || !matches!(pml, PML | STRICT_PML) {
            return Err(PptxError::InvalidSlide {
                part: reference.part_name().into(),
                message: "expected PresentationML sld root".into(),
            });
        }
        let mut parser = Parser {
            package,
            images,
            reference,
            warnings,
            pml,
            dml: if pml == PML { DML } else { STRICT_DML },
        };
        let mut slide = Slide {
            background: None,
            shapes: vec![],
        };
        parser.attributes(root, &["show", "showMasterSp", "showMasterPhAnim"]);
        for attr in ["showMasterSp", "showMasterPhAnim"] {
            if root.attribute(attr).is_some() {
                parser.warn(format!("{attr}: master content is not resolved"));
            }
        }
        if let Some(common) = parser.single(root, pml, "cSld")? {
            parser.attributes(common, &["name"]);
            if let Some(bg) = parser.single(common, pml, "bg")? {
                parser.attributes(bg, &[]);
                if let Some(properties) = parser.single(bg, pml, "bgPr")? {
                    parser.attributes(properties, &[]);
                    slide.background = parser.fill(properties)?;
                    parser.other_children(
                        properties,
                        &[
                            (parser.dml, "solidFill"),
                            (parser.dml, "noFill"),
                            (parser.dml, "gradFill"),
                            (parser.dml, "blipFill"),
                            (parser.dml, "pattFill"),
                            (parser.dml, "grpFill"),
                        ],
                    );
                }
                parser.other_children(bg, &[(pml, "bgPr")]);
            }
            if let Some(tree) = parser.single(common, pml, "spTree")? {
                parser.attributes(tree, &[]);
                // The root group container is structural; transforms of its
                // coordinate space are deliberately unsupported.
                let group = parser.single(tree, pml, "grpSpPr")?;
                let transformed = if let Some(group) = group {
                    !parser.structural_group(group)?
                } else {
                    false
                };
                if transformed {
                    parser.warn("root group properties: shape tree omitted".into());
                } else {
                    for node in tree.children().filter(|node| node.is_element()) {
                        match (node.tag_name().namespace(), node.tag_name().name()) {
                            (Some(ns), "pic") if ns == pml => {
                                if let Some(image) = parser.picture(node)? {
                                    slide.shapes.push(Shape::Image(image));
                                }
                            }
                            (Some(ns), "sp" | "cxnSp") if ns == pml => {
                                slide.shapes.extend(parser.shape(node)?);
                            }
                            (Some(ns), "nvGrpSpPr" | "grpSpPr") if ns == pml => {}
                            _ => parser.unsupported(node),
                        }
                    }
                }
            }
            parser.other_children(common, &[(pml, "bg"), (pml, "spTree")]);
        }
        parser.other_children(root, &[(pml, "cSld")]);
        Ok(slide)
    })
}

struct Parser<'a> {
    package: &'a OoxmlPackage,
    images: &'a mut BTreeMap<String, Arc<ImageResource>>,
    reference: &'a SlideRef,
    warnings: &'a mut Vec<PptxWarning>,
    pml: &'a str,
    dml: &'static str,
}

impl Parser<'_> {
    fn picture(&mut self, node: Node<'_, '_>) -> Result<Option<ImageOp>, PptxError> {
        let pml = self.pml;
        let dml = self.dml;
        let rel = if pml == PML { REL } else { STRICT_REL };
        self.attributes(node, &[]);
        self.other_children(node, &[(pml, "nvPicPr"), (pml, "blipFill"), (pml, "spPr")]);
        if let Some(nonvisual) = self.single(node, pml, "nvPicPr")? {
            self.attributes(nonvisual, &[]);
            self.other_children(
                nonvisual,
                &[(pml, "cNvPr"), (pml, "cNvPicPr"), (pml, "nvPr")],
            );
            if let Some(properties) = self.single(nonvisual, pml, "cNvPr")? {
                self.attributes(properties, &["id", "name", "descr", "title"]);
                self.other_children(properties, &[]);
                if properties.attribute("hidden").is_some() {
                    self.warn("hidden picture: picture omitted".into());
                    return Ok(None);
                }
            }
            if let Some(properties) = self.single(nonvisual, pml, "cNvPicPr")? {
                self.attributes(properties, &["preferRelativeResize"]);
                // Aspect locks constrain editing, not rendering into the explicit xfrm.
                if let Some(locks) = self.single(properties, dml, "picLocks")? {
                    self.attributes(locks, &["noChangeAspect"]);
                    self.leaf(locks)?;
                }
                self.other_children(properties, &[(dml, "picLocks")]);
            }
            if let Some(properties) = self.single(nonvisual, pml, "nvPr")? {
                self.attributes(properties, &[]);
                self.other_children(properties, &[]);
                if properties
                    .children()
                    .any(|child| child.has_tag_name((pml, "ph")))
                {
                    self.warn("placeholder picture: picture omitted".into());
                    return Ok(None);
                }
            }
        }
        let Some(fill) = self.single(node, pml, "blipFill")? else {
            self.warn("pic missing blipFill: picture omitted".into());
            return Ok(None);
        };
        self.attributes(fill, &["dpi", "rotWithShape"]);
        let blip = self
            .single(fill, dml, "blip")?
            .ok_or_else(|| self.invalid("picture missing blip"))?;
        self.single(fill, dml, "stretch")?;
        self.single(fill, dml, "srcRect")?;
        let Some(id) = blip.attribute((rel, "embed")) else {
            if blip.attribute((rel, "link")).is_some() {
                self.warn("external image link: picture omitted (never fetched)".into());
                return Ok(None);
            }
            return Err(self.invalid("picture missing embedded image reference"));
        };
        if id.is_empty() || id.trim() != id {
            return Err(self.invalid("invalid embedded image reference"));
        }
        for attr in blip.attributes() {
            if !((attr.namespace() == Some(rel) && attr.name() == "embed")
                || (attr.namespace().is_none() && attr.name() == "cstate"))
            {
                self.warn(format!(
                    "blip@{}",
                    xml::expanded(attr.namespace(), attr.name())
                ));
            }
        }
        let relationship = self
            .package
            .relationships(Some(self.reference.part_name()))?
            .iter()
            .find(|relationship| relationship.id == id)
            .ok_or_else(|| self.invalid("missing image relationship"))?;
        if !is_image_relationship(&relationship.relationship_type) {
            return Err(self.invalid("picture relationship type is not image"));
        }
        let RelationshipTarget::Internal(target) = &relationship.target else {
            self.warn("external image relationship: picture omitted (never fetched)".into());
            return Ok(None);
        };
        let media = self.package.part(target)?;
        let Some(format) = ImageFormat::detect(media.bytes()) else {
            if matches!(media.content_type(), "image/png" | "image/jpeg") {
                return Err(self.invalid("malformed PNG/JPEG media signature"));
            }
            self.warn(format!(
                "unsupported image format {}: picture omitted",
                xml::expanded(None, media.content_type())
            ));
            return Ok(None);
        };
        let mut supported = true;
        self.element_only(blip)?;
        for child in blip.children().filter(|child| child.is_element()) {
            self.unsupported(child);
            supported = false;
        }
        // Empty stretch/fillRect is the common full-image mapping to xfrm.
        for child in fill.children().filter(|child| child.is_element()) {
            if child == blip {
                continue;
            }
            if child.has_tag_name((dml, "stretch")) {
                self.attributes(child, &[]);
                let destination = self.single(child, dml, "fillRect")?;
                if child.attributes().len() != 0
                    || child.children().filter(|entry| entry.is_element()).count() != 1
                    || destination.is_none_or(|rect| {
                        rect.attributes().len() != 0
                            || rect.children().any(|entry| entry.is_element())
                    })
                {
                    self.warn("stretch destination rectangle: picture omitted".into());
                    supported = false;
                }
                if let Some(destination) = destination {
                    self.element_only(destination)?;
                }
            } else {
                self.unsupported(child); // srcRect, tile, extensions
                supported = false;
            }
        }
        let Some(properties) = self.single(node, pml, "spPr")? else {
            self.warn("inherited picture properties: picture omitted".into());
            return Ok(None);
        };
        self.attributes(properties, &[]);
        if properties
            .attribute("bwMode")
            .is_some_and(|mode| mode != "auto")
        {
            self.warn("picture black/white mode: picture omitted".into());
            supported = false;
        }
        for child in properties.children().filter(|child| child.is_element()) {
            if child.has_tag_name((dml, "xfrm")) {
                continue;
            }
            if child.has_tag_name((dml, "prstGeom")) && child.attribute("prst") == Some("rect") {
                self.attributes(child, &["prst"]);
                self.element_only(child)?;
                for adjustment in child.children().filter(|entry| entry.is_element()) {
                    self.attributes(adjustment, &[]);
                    self.element_only(adjustment)?;
                    if !adjustment.has_tag_name((dml, "avLst"))
                        || adjustment.children().any(|entry| entry.is_element())
                    {
                        self.unsupported(adjustment);
                        supported = false;
                    }
                }
            } else if child.has_tag_name((dml, "ln"))
                && child.children().filter(|entry| entry.is_element()).count() == 1
                && child
                    .children()
                    .any(|entry| entry.has_tag_name((dml, "noFill")))
            {
                self.attributes(child, &[]);
                let no_fill = self
                    .single(child, dml, "noFill")?
                    .ok_or_else(|| self.invalid("missing noFill"))?;
                self.attributes(no_fill, &[]);
                self.leaf(no_fill)?;
            } else {
                self.unsupported(child);
                supported = false;
            }
        }
        let Some(transform) = self.single(properties, dml, "xfrm")? else {
            self.warn("inherited picture transform: picture omitted".into());
            return Ok(None);
        };
        self.attributes(transform, &["rot", "flipH", "flipV"]);
        if transform.attribute("rot").is_some()
            && self.integer(transform, "rot", i64::from(i32::MIN), i64::from(i32::MAX))? != 0
        {
            self.warn("picture rotation: picture omitted".into());
            supported = false;
        }
        for name in ["flipH", "flipV"] {
            if let Some(value) = transform.attribute(name) {
                match value {
                    "0" | "false" => {}
                    "1" | "true" => {
                        self.warn(format!("picture {name}: picture omitted"));
                        supported = false;
                    }
                    _ => return Err(self.invalid(format!("invalid boolean {name}"))),
                }
            }
        }
        self.other_children(transform, &[(dml, "off"), (dml, "ext")]);
        let offset = self
            .single(transform, dml, "off")?
            .ok_or_else(|| self.invalid("picture xfrm missing off"))?;
        let extent = self
            .single(transform, dml, "ext")?
            .ok_or_else(|| self.invalid("picture xfrm missing ext"))?;
        self.attributes(offset, &["x", "y"]);
        self.attributes(extent, &["cx", "cy"]);
        self.leaf(offset)?;
        self.leaf(extent)?;
        let rect = Rect {
            origin: Point {
                x: emu_to_points(self.integer(
                    offset,
                    "x",
                    -27_273_042_329_600,
                    27_273_042_316_900,
                )?),
                y: emu_to_points(self.integer(
                    offset,
                    "y",
                    -27_273_042_329_600,
                    27_273_042_316_900,
                )?),
            },
            size: Size {
                width: emu_to_points(self.integer(extent, "cx", 0, 27_273_042_316_900)?),
                height: emu_to_points(self.integer(extent, "cy", 0, 27_273_042_316_900)?),
            },
        };
        if !supported {
            return Ok(None);
        }
        let image = self
            .images
            .entry(target.clone())
            .or_insert_with(|| {
                Arc::new(ImageResource {
                    format,
                    bytes: Arc::from(media.bytes()),
                })
            })
            .clone();
        Ok(Some(ImageOp { rect, image }))
    }

    // Office writers commonly serialize an all-zero root xfrm. It does not
    // establish a transformed coordinate space and is safe as scaffolding.
    fn structural_group(&mut self, group: Node<'_, '_>) -> Result<bool, PptxError> {
        self.attributes(group, &[]);
        self.element_only(group)?;
        let dml = self.dml;
        let transform = self.single(group, dml, "xfrm")?;
        if group.children().filter(|child| child.is_element()).count()
            != usize::from(transform.is_some())
        {
            return Ok(false);
        }
        let Some(transform) = transform else {
            return Ok(true);
        };
        if transform.attributes().len() != 0 {
            return Ok(false);
        }
        self.element_only(transform)?;
        for child in transform.children().filter(|child| child.is_element()) {
            let attributes = match (child.tag_name().namespace(), child.tag_name().name()) {
                (Some(ns), "off" | "chOff") if ns == dml => ["x", "y"],
                (Some(ns), "ext" | "chExt") if ns == dml => ["cx", "cy"],
                _ => return Ok(false),
            };
            self.leaf(child)?;
            if child.attributes().len() != 2 {
                return Ok(false);
            }
            for name in attributes {
                if self.integer(child, name, i64::MIN, i64::MAX)? != 0 {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    fn invalid(&self, message: impl Into<String>) -> PptxError {
        PptxError::InvalidSlide {
            part: self.reference.part_name().into(),
            message: message.into(),
        }
    }

    fn warn(&mut self, name: String) {
        self.warnings.push(PptxWarning::UnsupportedSlideContent {
            slide_id: self.reference.id(),
            part: self.reference.part_name().into(),
            name,
        });
    }

    fn unsupported(&mut self, node: Node<'_, '_>) {
        self.warn(xml::expanded(
            node.tag_name().namespace(),
            node.tag_name().name(),
        ));
    }

    fn attributes(&mut self, node: Node<'_, '_>, known: &[&str]) {
        for attr in node.attributes() {
            if attr.namespace().is_some() || !known.contains(&attr.name()) {
                self.warn(format!(
                    "{}@{}",
                    xml::expanded(None, node.tag_name().name()),
                    xml::expanded(attr.namespace(), attr.name())
                ));
            }
        }
    }

    fn other_children(&mut self, node: Node<'_, '_>, known: &[(&str, &str)]) {
        for child in node.children().filter(|node| node.is_element()) {
            if !known.contains(&(
                child.tag_name().namespace().unwrap_or(""),
                child.tag_name().name(),
            )) {
                self.unsupported(child);
            }
        }
    }

    fn single<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        ns: &str,
        name: &str,
    ) -> Result<Option<Node<'a, 'input>>, PptxError> {
        self.element_only(node)?;
        let mut matches = node
            .children()
            .filter(|child| child.has_tag_name((ns, name)));
        let first = matches.next();
        if matches.next().is_some() {
            return Err(self.invalid(format!("duplicate {name}")));
        }
        Ok(first)
    }

    fn element_only(&self, node: Node<'_, '_>) -> Result<(), PptxError> {
        if node.children().any(|child| {
            child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty())
        }) {
            return Err(self.invalid(format!("unexpected text in {}", node.tag_name().name())));
        }
        Ok(())
    }

    fn leaf(&self, node: Node<'_, '_>) -> Result<(), PptxError> {
        self.element_only(node)?;
        if node.children().any(|child| child.is_element()) {
            return Err(self.invalid(format!("unexpected child of {}", node.tag_name().name())));
        }
        Ok(())
    }

    fn integer(
        &self,
        node: Node<'_, '_>,
        name: &str,
        min: i64,
        max: i64,
    ) -> Result<i64, PptxError> {
        let value = node
            .attribute(name)
            .ok_or_else(|| self.invalid(format!("missing {name}")))?
            .trim();
        let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(self.invalid(format!("invalid integer {name}")));
        }
        let number: i64 = value
            .parse()
            .map_err(|_| self.invalid(format!("overflowing integer {name}")))?;
        if !(min..=max).contains(&number) {
            return Err(self.invalid(format!("out-of-range {name}")));
        }
        Ok(number)
    }

    fn fill(&mut self, parent: Node<'_, '_>) -> Result<Option<Fill>, PptxError> {
        self.element_only(parent)?;
        let mut fill = None;
        for node in parent.children().filter(|node| node.is_element()) {
            if node.tag_name().namespace() != Some(self.dml)
                || !matches!(
                    node.tag_name().name(),
                    "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                )
            {
                continue;
            }
            if fill.is_some() {
                return Err(self.invalid("multiple fill declarations"));
            }
            fill = Some(node);
        }
        let Some(node) = fill else {
            self.warn(format!(
                "{}: inherited fill is not resolved",
                parent.tag_name().name()
            ));
            return Ok(None);
        };
        self.attributes(node, &[]);
        match node.tag_name().name() {
            "noFill" => {
                self.leaf(node)?;
                Ok(Some(Fill::None))
            }
            "solidFill" => {
                self.element_only(node)?;
                let mut colors = node.children().filter(|child| child.is_element());
                let color = colors
                    .next()
                    .ok_or_else(|| self.invalid("solidFill has no color"))?;
                if colors.next().is_some() {
                    return Err(self.invalid("solidFill has multiple colors"));
                }
                if !color.has_tag_name((self.dml, "srgbClr")) {
                    self.unsupported(color);
                    return Ok(None);
                }
                self.attributes(color, &["val"]);
                let raw = color
                    .attribute("val")
                    .ok_or_else(|| self.invalid("missing sRGB val"))?;
                if raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(self.invalid("sRGB val must be six hexadecimal digits"));
                }
                let number =
                    u32::from_str_radix(raw, 16).map_err(|_| self.invalid("invalid sRGB color"))?;
                self.element_only(color)?;
                if color.children().any(|child| child.is_element()) {
                    // Alpha, tint, shade and other transforms cannot be silently
                    // treated as the base color.
                    self.other_children(color, &[]);
                    return Ok(None);
                }
                Ok(Some(Fill::Solid(Color::rgb(
                    (number >> 16) as u8,
                    (number >> 8) as u8,
                    number as u8,
                ))))
            }
            _ => {
                self.unsupported(node);
                Ok(None)
            }
        }
    }

    fn shape(&mut self, node: Node<'_, '_>) -> Result<Vec<Shape>, PptxError> {
        self.attributes(node, &[]);
        let pml = self.pml;
        let dml = self.dml;
        let text_body = self.single(node, pml, "txBody")?;
        // Placeholder geometry may be inherited; never guess it.
        let connector = node.has_tag_name((pml, "cxnSp"));
        let nonvisual_name = if connector { "nvCxnSpPr" } else { "nvSpPr" };
        if let Some(nonvisual) = self.single(node, pml, nonvisual_name)? {
            if connector {
                if let Some(properties) = self.single(nonvisual, pml, "cNvCxnSpPr")? {
                    self.attributes(properties, &[]);
                    self.other_children(properties, &[]);
                }
            }
            if nonvisual
                .descendants()
                .any(|child| child.has_tag_name((pml, "ph")))
            {
                if text_body.is_none() {
                    self.warn("placeholder: shape omitted".into());
                    return Ok(vec![]);
                }
                self.warn(
                    "text placeholder inheritance: using explicit transform and text fallbacks"
                        .into(),
                );
            }
        }
        self.other_children(
            node,
            &[(pml, nonvisual_name), (pml, "spPr"), (pml, "txBody")],
        );
        let Some(properties) = self.single(node, pml, "spPr")? else {
            self.warn("missing shape properties: shape omitted".into());
            return Ok(vec![]);
        };
        self.attributes(properties, &[]);
        self.other_children(
            properties,
            &[
                (dml, "xfrm"),
                (dml, "prstGeom"),
                (dml, "solidFill"),
                (dml, "noFill"),
                (dml, "gradFill"),
                (dml, "blipFill"),
                (dml, "pattFill"),
                (dml, "grpFill"),
                (dml, "ln"),
            ],
        );
        let geometry = self.single(properties, dml, "prstGeom")?;
        let preset = if let Some(geometry) = geometry {
            self.attributes(geometry, &["prst"]);
            self.element_only(geometry)?;
            let preset = geometry.attribute("prst").unwrap_or("");
            if !matches!(preset, "rect" | "ellipse" | "line") || (connector && preset != "line") {
                self.warn("unsupported preset geometry: shape omitted".into());
                return Ok(vec![]);
            }
            for child in geometry.children().filter(|child| child.is_element()) {
                if !child.has_tag_name((dml, "avLst"))
                    || child.children().any(|entry| entry.is_element())
                {
                    self.unsupported(child);
                    return Ok(vec![]);
                }
                self.attributes(child, &[]);
                self.leaf(child)?;
            }
            preset
        } else if text_body.is_some() && self.single(properties, dml, "custGeom")?.is_none() {
            self.warn("text-only shape: inherited geometry is not resolved".into());
            "text"
        } else {
            self.warn("non-preset or inherited geometry: shape omitted".into());
            return Ok(vec![]);
        };
        let Some(transform) = self.single(properties, dml, "xfrm")? else {
            self.warn("inherited transform: shape omitted".into());
            return Ok(vec![]);
        };
        self.attributes(transform, &["rot", "flipH", "flipV"]);
        if transform.attribute("rot").is_some()
            && self.integer(transform, "rot", i64::from(i32::MIN), i64::from(i32::MAX))? != 0
        {
            self.warn("rotation: shape omitted".into());
            return Ok(vec![]);
        }
        let mut flips = [false; 2];
        for (index, name) in ["flipH", "flipV"].into_iter().enumerate() {
            if let Some(value) = transform.attribute(name) {
                match value {
                    "0" | "false" => {}
                    "1" | "true" => {
                        if preset != "line" {
                            self.warn(format!("{name}: shape omitted"));
                            return Ok(vec![]);
                        }
                        flips[index] = true;
                    }
                    _ => return Err(self.invalid(format!("invalid boolean {name}"))),
                }
            }
        }
        self.other_children(transform, &[(dml, "off"), (dml, "ext")]);
        let offset = self
            .single(transform, dml, "off")?
            .ok_or_else(|| self.invalid("xfrm missing off"))?;
        let extent = self
            .single(transform, dml, "ext")?
            .ok_or_else(|| self.invalid("xfrm missing ext"))?;
        self.attributes(offset, &["x", "y"]);
        self.leaf(offset)?;
        self.attributes(extent, &["cx", "cy"]);
        self.leaf(extent)?;
        let rect = Rect {
            origin: Point {
                x: emu_to_points(self.integer(
                    offset,
                    "x",
                    -27_273_042_329_600,
                    27_273_042_316_900,
                )?),
                y: emu_to_points(self.integer(
                    offset,
                    "y",
                    -27_273_042_329_600,
                    27_273_042_316_900,
                )?),
            },
            size: Size {
                width: emu_to_points(self.integer(extent, "cx", 0, 27_273_042_316_900)?),
                height: emu_to_points(self.integer(extent, "cy", 0, 27_273_042_316_900)?),
            },
        };
        let fill = if preset == "line" {
            // An open line has no interior. Still diagnose explicit unsupported paint.
            if properties.children().any(|child| {
                child.is_element()
                    && child.tag_name().namespace() == Some(dml)
                    && matches!(
                        child.tag_name().name(),
                        "solidFill" | "noFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                    )
            }) {
                self.fill(properties)?;
            }
            None
        } else {
            self.fill(properties)?
        };
        let stroke = if let Some(line) = self.single(properties, dml, "ln")? {
            self.stroke(line)?
        } else {
            self.warn("inherited outline is not resolved".into());
            None
        };
        let shape = match preset {
            "ellipse" => Shape::Ellipse {
                bounds: rect,
                fill,
                stroke,
            },
            "line" => {
                let mut start = rect.origin;
                let mut end = Point {
                    x: start.x + rect.size.width,
                    y: start.y + rect.size.height,
                };
                if flips[0] {
                    std::mem::swap(&mut start.x, &mut end.x);
                }
                if flips[1] {
                    std::mem::swap(&mut start.y, &mut end.y);
                }
                Shape::Line { start, end, stroke }
            }
            _ => Shape::Rectangle { rect, fill, stroke },
        };
        let mut shapes = vec![];
        if preset != "text" {
            shapes.push(shape);
        }
        if let Some(body) = text_body {
            if connector {
                self.warn("connector text is not rendered".into());
            } else if let Some(text) = self.text_box(body, rect)? {
                if text.layout().1 {
                    self.warn("legacy text box overflow: unwrapped text exceeds the inner box; configured-font overflow is reported after wrapping".into());
                }
                shapes.push(Shape::TextBox(text));
            }
        }
        Ok(shapes)
    }

    fn stroke(&mut self, line: Node<'_, '_>) -> Result<Option<Stroke>, PptxError> {
        self.attributes(line, &["w", "cap", "cmpd", "algn"]);
        let fill = self.fill(line)?;
        let width = if line.attribute("w").is_some() {
            emu_to_points(self.integer(line, "w", 0, 20_116_800)?)
        } else {
            if matches!(fill, Some(Fill::Solid(_))) {
                self.warn("inherited outline width: using 0.75-point base default".into());
            }
            0.75
        };
        let mut supported = true;
        for (name, default) in [("cap", "flat"), ("cmpd", "sng"), ("algn", "ctr")] {
            if line.attribute(name).is_some_and(|value| value != default) {
                self.warn(format!("outline {name}: outline omitted"));
                supported = false;
            }
        }
        for child in line.children().filter(|node| node.is_element()) {
            if child.tag_name().namespace() == Some(self.dml)
                && matches!(
                    child.tag_name().name(),
                    "solidFill" | "noFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                )
            {
                continue;
            }
            if child.has_tag_name((self.dml, "prstDash")) && child.attribute("val") == Some("solid")
            {
                self.attributes(child, &["val"]);
                self.leaf(child)?;
                continue;
            }
            self.unsupported(child);
            supported = false;
        }
        if supported && width > 0.0 {
            if let Some(Fill::Solid(color)) = fill {
                return Ok(Some(Stroke { color, width }));
            }
        }
        Ok(None)
    }
}
