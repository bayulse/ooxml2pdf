//! Private bounded theme/color adapter shared by every slide paint path.
use std::collections::BTreeSet;

use o2p_core::Color;
use o2p_ooxml::{OoxmlPackage, RelationshipTarget};
use roxmltree::Node;

use crate::theme::{ColorBase, ColorTransformKind, ColorValue};
use crate::{ParseLimits, PptxError, PptxWarning, Theme, ThemeColorSlot, ThemeFonts, xml};

const DML: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const STRICT_DML: &str = "http://purl.oclc.org/ooxml/drawingml/main";
const THEME_TYPE: &str = "application/vnd.openxmlformats-officedocument.theme+xml";

pub(crate) fn load(
    package: &OoxmlPackage,
    presentation: &str,
    limits: ParseLimits,
    warnings: &mut Vec<PptxWarning>,
) -> Result<Option<Theme>, PptxError> {
    let mut relationships = package
        .relationships(Some(presentation))?
        .iter()
        .filter(|r| {
            matches!(
                r.relationship_type.as_str(),
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme"
                    | "http://purl.oclc.org/ooxml/officeDocument/relationships/theme"
            )
        });
    let Some(relationship) = relationships.next() else {
        return Ok(None);
    };
    let mut context = Context {
        color_map: None,
        part: presentation,
        slide_id: None,
        warnings,
    };
    if relationships.next().is_some() {
        return Err(context.invalid("multiple presentation theme relationships"));
    }
    let RelationshipTarget::Internal(target) = &relationship.target else {
        context.warn("external theme relationship: theme unavailable (never fetched)".into());
        return Ok(None);
    };
    let part = package.part(target)?;
    context.part = target;
    if part.content_type() != THEME_TYPE {
        return Err(context.invalid("theme relationship target has wrong content type"));
    }
    xml::with_document(part.bytes(), target, limits, |document| {
        parse(document.root_element(), &mut context).map(Some)
    })
}

pub(crate) struct Context<'a> {
    pub color_map: Option<&'a crate::ColorMap>,
    pub part: &'a str,
    pub slide_id: Option<u32>,
    pub warnings: &'a mut Vec<PptxWarning>,
}

impl Context<'_> {
    pub(crate) fn invalid(&self, message: impl Into<String>) -> PptxError {
        if self.slide_id.is_some() {
            PptxError::InvalidSlide {
                part: self.part.into(),
                message: message.into(),
            }
        } else {
            PptxError::InvalidTheme {
                part: self.part.into(),
                message: message.into(),
            }
        }
    }

    pub fn warn(&mut self, name: String) {
        self.warnings.push(match self.slide_id {
            Some(slide_id) => PptxWarning::UnsupportedSlideContent {
                slide_id,
                part: self.part.into(),
                name,
            },
            None => PptxWarning::UnsupportedTheme {
                part: self.part.into(),
                name,
            },
        });
    }

    pub(crate) fn attributes(&mut self, node: Node<'_, '_>, known: &[&str]) {
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

    pub(crate) fn element_only(&self, node: Node<'_, '_>) -> Result<(), PptxError> {
        if node
            .children()
            .any(|n| n.is_text() && n.text().is_some_and(|s| !s.trim().is_empty()))
        {
            return Err(self.invalid(format!(
                "unexpected text in {}",
                xml::expanded(None, node.tag_name().name())
            )));
        }
        Ok(())
    }

    pub(crate) fn leaf(&self, node: Node<'_, '_>) -> Result<(), PptxError> {
        self.element_only(node)?;
        if node.children().any(|n| n.is_element()) {
            return Err(self.invalid("unexpected nested content"));
        }
        Ok(())
    }

    pub(crate) fn single<'a, 'input>(
        &self,
        parent: Node<'a, 'input>,
        ns: &str,
        name: &str,
    ) -> Result<Node<'a, 'input>, PptxError> {
        self.element_only(parent)?;
        let mut nodes = parent.children().filter(|n| n.has_tag_name((ns, name)));
        let first = nodes
            .next()
            .ok_or_else(|| self.invalid(format!("missing {name}")))?;
        if nodes.next().is_some() {
            return Err(self.invalid(format!("duplicate {name}")));
        }
        Ok(first)
    }

    pub(crate) fn other_children(&mut self, node: Node<'_, '_>, ns: &str, known: &[&str]) {
        for child in node.children().filter(|n| n.is_element()) {
            if child.tag_name().namespace() != Some(ns) || !known.contains(&child.tag_name().name())
            {
                self.warn(xml::expanded(
                    child.tag_name().namespace(),
                    child.tag_name().name(),
                ));
            }
        }
    }

    pub(crate) fn string<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        name: &str,
    ) -> Result<&'a str, PptxError> {
        let value = node
            .attribute(name)
            .ok_or_else(|| self.invalid(format!("missing {name}")))?;
        if value.len() > 256 || value.chars().any(char::is_control) {
            return Err(self.invalid(format!("invalid {name}")));
        }
        Ok(value)
    }

    pub(crate) fn hex(&self, raw: &str) -> Result<Color, PptxError> {
        if raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(self.invalid("sRGB val/lastClr must be six hexadecimal digits"));
        }
        let n = u32::from_str_radix(raw, 16).map_err(|_| self.invalid("invalid sRGB color"))?;
        Ok(Color::rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
    }
}

fn parse(root: Node<'_, '_>, context: &mut Context<'_>) -> Result<Theme, PptxError> {
    let ns = root.tag_name().namespace().unwrap_or("");
    if root.tag_name().name() != "theme" || !matches!(ns, DML | STRICT_DML) {
        return Err(context.invalid("expected DrawingML theme root"));
    }
    context.attributes(root, &["name"]);
    let elements = context.single(root, ns, "themeElements")?;
    context.attributes(elements, &[]);
    context.other_children(root, ns, &["themeElements"]);
    context.other_children(elements, ns, &["clrScheme", "fontScheme", "fmtScheme"]);
    let colors = context.single(elements, ns, "clrScheme")?;
    context.attributes(colors, &["name"]);
    context.element_only(colors)?;
    let mut theme = Theme::default();
    let mut seen = BTreeSet::new();
    for entry in colors.children().filter(|n| n.is_element()) {
        let Some(slot) = ThemeColorSlot::from_name(entry.tag_name().name())
            .filter(|_| entry.tag_name().namespace() == Some(ns))
        else {
            context.warn(xml::expanded(
                entry.tag_name().namespace(),
                entry.tag_name().name(),
            ));
            continue;
        };
        if !seen.insert(slot) {
            return Err(context.invalid("duplicate color-scheme slot"));
        }
        context.attributes(entry, &[]);
        context.element_only(entry)?;
        let mut children = entry.children().filter(|n| n.is_element());
        let value = children
            .next()
            .ok_or_else(|| context.invalid("empty color-scheme slot"))?;
        if children.next().is_some() {
            return Err(context.invalid("multiple color-scheme definitions"));
        }
        if let Some(value) = color(value, ns, None, context)? {
            theme.colors.colors.insert(slot, value);
        }
    }
    for name in [
        "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ] {
        if !seen.contains(&ThemeColorSlot::from_name(name).expect("standard slot")) {
            context.warn(format!("missing color-scheme slot {name}: unavailable"));
        }
    }
    let fonts = context.single(elements, ns, "fontScheme")?;
    context.attributes(fonts, &["name"]);
    context.other_children(fonts, ns, &["majorFont", "minorFont"]);
    theme.fonts.major = font_collection(context.single(fonts, ns, "majorFont")?, ns, context)?;
    theme.fonts.minor = font_collection(context.single(fonts, ns, "minorFont")?, ns, context)?;
    if elements
        .children()
        .any(|n| n.has_tag_name((ns, "fmtScheme")))
    {
        theme.styles =
            crate::styles_xml::matrix(context.single(elements, ns, "fmtScheme")?, ns, context)?;
    }
    Ok(theme)
}

fn font_collection(
    node: Node<'_, '_>,
    ns: &str,
    context: &mut Context<'_>,
) -> Result<ThemeFonts, PptxError> {
    context.attributes(node, &[]);
    context.element_only(node)?;
    let mut fonts = ThemeFonts::default();
    let latin = context.single(node, ns, "latin")?;
    for (name, target) in [
        ("latin", &mut fonts.latin),
        ("ea", &mut fonts.east_asian),
        ("cs", &mut fonts.complex_script),
    ] {
        if node.children().any(|n| n.has_tag_name((ns, name))) {
            let entry = if name == "latin" {
                latin
            } else {
                context.single(node, ns, name)?
            };
            context.attributes(entry, &["typeface", "panose", "pitchFamily", "charset"]);
            context.leaf(entry)?;
            let family = context.string(entry, "typeface")?;
            if family.starts_with('+') {
                return Err(context.invalid("theme font definition must be a concrete family"));
            }
            *target = (!family.trim().is_empty()).then(|| family.to_owned());
        }
    }
    for entry in node.children().filter(|n| n.has_tag_name((ns, "font"))) {
        context.attributes(entry, &["script", "typeface"]);
        context.leaf(entry)?;
        let script = context.string(entry, "script")?;
        let family = context.string(entry, "typeface")?;
        if script.is_empty() || family.starts_with('+') {
            return Err(context.invalid("invalid script font mapping"));
        }
        if fonts.scripts.insert(script.into(), family.into()).is_some() {
            return Err(context.invalid("duplicate script font mapping"));
        }
    }
    context.other_children(node, ns, &["latin", "ea", "cs", "font"]);
    Ok(fonts)
}

/// One color adapter for direct paints, theme definitions and style entries.
pub(crate) fn color(
    node: Node<'_, '_>,
    ns: &str,
    theme: Option<&Theme>,
    context: &mut Context<'_>,
) -> Result<Option<Color>, PptxError> {
    let value = color_value(node, ns, context.slide_id.is_some(), context)?;
    Ok(value.resolve(theme, None, context))
}

/// Normalize first; matrix colors need a reference's theme and phClr later.
pub(crate) fn color_value(
    node: Node<'_, '_>,
    ns: &str,
    allow_scheme: bool,
    context: &mut Context<'_>,
) -> Result<ColorValue, PptxError> {
    context.element_only(node)?;
    let mut base = if node.has_tag_name((ns, "srgbClr")) {
        context.attributes(node, &["val"]);
        ColorBase::Rgb(context.hex(context.string(node, "val")?)?)
    } else if node.has_tag_name((ns, "sysClr")) {
        context.attributes(node, &["val", "lastClr"]);
        context.string(node, "val")?;
        match node.attribute("lastClr") {
            Some(raw) => ColorBase::Rgb(context.hex(raw)?),
            None => {
                context.warn("sysClr without lastClr: color unavailable (no system lookup)".into());
                ColorBase::Unavailable
            }
        }
    } else if node.has_tag_name((ns, "schemeClr")) && allow_scheme {
        context.attributes(node, &["val"]);
        ColorBase::Scheme(context.string(node, "val")?.into())
    } else {
        context.warn(format!(
            "{}: unsupported color representation; color unavailable",
            xml::expanded(node.tag_name().namespace(), node.tag_name().name())
        ));
        ColorBase::Unavailable
    };
    let mut transforms = Vec::new();
    for transform in node.children().filter(|n| n.is_element()) {
        let name = transform.tag_name().name();
        if transform.tag_name().namespace() != Some(ns)
            || !matches!(name, "tint" | "shade" | "lumMod" | "lumOff")
        {
            context.warn(format!(
                "{}: unsupported color transform; color unavailable",
                xml::expanded(transform.tag_name().namespace(), name)
            ));
            base = ColorBase::Unavailable;
            continue;
        }
        context.attributes(transform, &["val"]);
        context.leaf(transform)?;
        let raw = context.string(transform, "val")?.trim();
        let digits = raw.strip_prefix(['+', '-']).unwrap_or(raw);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(context.invalid("invalid color-transform percentage"));
        }
        let value: i32 = raw
            .parse()
            .map_err(|_| context.invalid("overflowing color-transform percentage"))?;
        if (matches!(name, "tint" | "shade") && !(0..=100_000).contains(&value))
            || (name == "lumMod" && value < 0)
        {
            return Err(context.invalid("out-of-range color-transform percentage"));
        }
        transforms.push((
            match name {
                "tint" => ColorTransformKind::Tint,
                "shade" => ColorTransformKind::Shade,
                "lumMod" => ColorTransformKind::LumMod,
                "lumOff" => ColorTransformKind::LumOff,
                _ => unreachable!(),
            },
            value,
        ));
    }
    Ok(ColorValue { base, transforms })
}

pub(crate) fn font(
    token: &str,
    theme: Option<&Theme>,
    context: &mut Context<'_>,
) -> Option<String> {
    if !token.starts_with('+') {
        return (!token.trim().is_empty()).then(|| token.to_owned());
    }
    let resolved = theme.and_then(|theme| theme.resolve_font(token));
    if resolved.is_none() {
        context.warn(format!("theme font {} unresolved: using configured sans-serif; only explicit +mj-lt/+mn-lt are supported", xml::expanded(None, token)));
    }
    resolved.map(str::to_owned)
}
