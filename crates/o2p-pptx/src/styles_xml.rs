//! Bounded DrawingML format-style adapter; no XML types leave this crate.
use crate::styles::{StyleDefaults, StyleFill, StyleLine};
use crate::theme_xml::{Context, color_value};
use crate::{Fill, PptxError, StyleMatrix, Theme, emu_to_points, xml};
use o2p_core::Color;
use roxmltree::Node;

fn optional<'a, 'input>(
    node: Node<'a, 'input>,
    ns: &str,
    name: &str,
    context: &Context<'_>,
) -> Result<Option<Node<'a, 'input>>, PptxError> {
    context.element_only(node)?;
    if node.children().any(|n| n.has_tag_name((ns, name))) {
        context.single(node, ns, name).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn matrix(
    node: Node<'_, '_>,
    ns: &str,
    context: &mut Context<'_>,
) -> Result<StyleMatrix, PptxError> {
    context.attributes(node, &["name"]);
    context.element_only(node)?;
    context.other_children(
        node,
        ns,
        &[
            "fillStyleLst",
            "lnStyleLst",
            "bgFillStyleLst",
            "effectStyleLst",
        ],
    );
    let mut matrix = StyleMatrix::default();
    for (name, target) in [
        ("fillStyleLst", &mut matrix.fills),
        ("bgFillStyleLst", &mut matrix.backgrounds),
    ] {
        if let Some(list) = optional(node, ns, name, context)? {
            context.attributes(list, &[]);
            context.element_only(list)?;
            for entry in list.children().filter(|n| n.is_element()) {
                target.push(fill(entry, ns, context)?);
            }
        }
    }
    if let Some(list) = optional(node, ns, "lnStyleLst", context)? {
        context.attributes(list, &[]);
        context.element_only(list)?;
        for entry in list.children().filter(|n| n.is_element()) {
            matrix.lines.push(line(entry, ns, context)?);
        }
    }
    if let Some(effects) = optional(node, ns, "effectStyleLst", context)? {
        context.attributes(effects, &[]);
        context.element_only(effects)?;
        for entry in effects.children().filter(|n| n.is_element()) {
            if !entry.has_tag_name((ns, "effectStyle")) {
                context.warn("unsupported effect style entry".into());
                matrix.effects.push(None);
                continue;
            }
            context.attributes(entry, &[]);
            context.other_children(entry, ns, &["effectLst"]);
            matrix
                .effects
                .push(crate::effects::list(entry, ns, context)?.flatten());
        }
    }
    Ok(matrix)
}

fn fill(node: Node<'_, '_>, ns: &str, context: &mut Context<'_>) -> Result<StyleFill, PptxError> {
    context.element_only(node)?;
    context.attributes(node, &[]);
    if node.has_tag_name((ns, "noFill")) {
        context.leaf(node)?;
        return Ok(StyleFill::None);
    }
    if node.has_tag_name((ns, "solidFill")) {
        let mut colors = node.children().filter(|n| n.is_element());
        let color = colors
            .next()
            .ok_or_else(|| context.invalid("style solidFill has no color"))?;
        if colors.next().is_some() {
            return Err(context.invalid("style solidFill has multiple colors"));
        }
        return color_value(color, ns, true, context).map(StyleFill::Solid);
    }
    let name = xml::expanded(node.tag_name().namespace(), node.tag_name().name());
    context.warn(format!(
        "style-matrix {name}: unsupported fill; paint omitted when referenced"
    ));
    Ok(StyleFill::Unsupported(name))
}

fn line(node: Node<'_, '_>, ns: &str, context: &mut Context<'_>) -> Result<StyleLine, PptxError> {
    if !node.has_tag_name((ns, "ln")) {
        let name = xml::expanded(node.tag_name().namespace(), node.tag_name().name());
        context.warn(format!("style-matrix {name}: unsupported line entry"));
        return Ok(StyleLine {
            width: None,
            fill: StyleFill::Unsupported(name),
        });
    }
    context.attributes(node, &["w", "cap", "cmpd", "algn"]);
    context.element_only(node)?;
    let width = node
        .attribute("w")
        .map(|raw| {
            let raw = raw.trim();
            let digits = raw.strip_prefix('+').unwrap_or(raw);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(context.invalid("invalid style line width"));
            }
            let width: u32 = raw
                .parse()
                .map_err(|_| context.invalid("overflowing style line width"))?;
            if width > 20_116_800 {
                return Err(context.invalid("out-of-range style line width"));
            }
            Ok(width)
        })
        .transpose()?;
    let mut paint = None;
    let mut unsupported = None;
    for (name, default) in [("cap", "flat"), ("cmpd", "sng"), ("algn", "ctr")] {
        if node.attribute(name).is_some_and(|v| v != default) {
            unsupported = Some(format!("ln@{name}"));
        }
    }
    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().namespace() == Some(ns)
            && matches!(
                child.tag_name().name(),
                "solidFill" | "noFill" | "gradFill" | "pattFill" | "blipFill" | "grpFill"
            )
        {
            if paint.is_some() {
                return Err(context.invalid("multiple style line fills"));
            }
            paint = Some(fill(child, ns, context)?);
        } else if child.has_tag_name((ns, "prstDash")) && child.attribute("val") == Some("solid") {
            context.attributes(child, &["val"]);
            context.leaf(child)?;
        } else {
            let name = xml::expanded(child.tag_name().namespace(), child.tag_name().name());
            context.warn(format!(
                "style-matrix {name}: unsupported line decoration; stroke omitted"
            ));
            unsupported = Some(name);
        }
    }
    let fill = if let Some(name) = unsupported {
        context.warn(format!(
            "style-matrix {name}: unsupported line semantics; stroke omitted"
        ));
        StyleFill::Unsupported(name)
    } else {
        paint.unwrap_or_else(|| StyleFill::Unsupported("ln without fill".into()))
    };
    Ok(StyleLine { width, fill })
}

fn index(node: Node<'_, '_>, context: &mut Context<'_>) -> Option<u32> {
    let raw = node.attribute("idx").unwrap_or("").trim();
    let digits = raw.strip_prefix('+').unwrap_or(raw);
    let index = (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| raw.parse::<u32>().ok())
        .flatten();
    if index.is_none() {
        context.warn(format!(
            "style {}: malformed/missing idx {}; paint omitted",
            node.tag_name().name(),
            xml::expanded(None, raw)
        ));
    }
    index
}

fn reference_color(
    node: Node<'_, '_>,
    ns: &str,
    theme: Option<&Theme>,
    context: &mut Context<'_>,
) -> Result<Option<Color>, PptxError> {
    context.element_only(node)?;
    let mut children = node.children().filter(|n| n.is_element());
    let Some(color) = children.next() else {
        return Ok(None);
    };
    if children.next().is_some() {
        return Err(context.invalid("multiple style reference colors"));
    }
    crate::theme_xml::color(color, ns, theme, context)
}

pub(crate) fn fill_reference(
    node: Node<'_, '_>,
    ns: &str,
    theme: Option<&Theme>,
    context: &mut Context<'_>,
) -> Result<Option<Fill>, PptxError> {
    context.attributes(node, &["idx"]);
    let index = index(node, context);
    // Diagnose the reference itself before its color so source warnings identify bgRef.
    let entry = index.and_then(|i| match i {
        0 | 1000 => Some(&StyleFill::None),
        _ => theme.and_then(|t| t.styles.fill(i)),
    });
    if index.is_some() && entry.is_none() {
        context.warn(format!(
            "style {} idx {}: theme style unavailable/out of range; paint omitted",
            node.tag_name().name(),
            index.unwrap_or(0)
        ));
    }
    let color = reference_color(node, ns, theme, context)?;
    Ok(entry.and_then(|fill| fill.resolve(theme, color, context)))
}

pub(crate) fn references(
    node: Node<'_, '_>,
    ns: &str,
    theme: Option<&Theme>,
    context: &mut Context<'_>,
) -> Result<StyleDefaults, PptxError> {
    context.attributes(node, &[]);
    context.element_only(node)?;
    context.other_children(node, ns, &["fillRef", "lnRef", "fontRef", "effectRef"]);
    let mut defaults = StyleDefaults::default();
    if let Some(fill) = optional(node, ns, "fillRef", context)? {
        defaults.fill = Some(fill_reference(fill, ns, theme, context)?);
    }
    if let Some(line) = optional(node, ns, "lnRef", context)? {
        context.attributes(line, &["idx"]);
        let index = index(line, context);
        let color = reference_color(line, ns, theme, context)?;
        defaults.line_width = Some(0.75);
        defaults.line_color = Some(None);
        if index == Some(0) {
            // No referenced paint; a local color can still use the base width.
        } else if let Some(entry) = index.and_then(|i| theme.and_then(|t| t.styles.line(i))) {
            defaults.line_width = Some(entry.width.map_or(0.75, |w| emu_to_points(i64::from(w))));
            defaults.line_color = Some(match entry.fill.resolve(theme, color, context) {
                Some(Fill::Solid(c)) => Some(c),
                _ => None,
            });
        } else if let Some(index) = index {
            context.warn(format!(
                "style lnRef idx {index}: theme style unavailable/out of range; stroke omitted"
            ));
        }
    }
    if let Some(font) = optional(node, ns, "fontRef", context)? {
        context.attributes(font, &["idx"]);
        let color = reference_color(font, ns, theme, context)?;
        if font.children().any(|n| n.is_element()) {
            defaults.text.color = Some(color);
        }
        defaults.text.family = Some(match font.attribute("idx") {
            Some("major") => crate::theme_xml::font("+mj-lt", theme, context),
            Some("minor") => crate::theme_xml::font("+mn-lt", theme, context),
            Some("none") => None,
            _ => {
                context.warn(
                    "style fontRef: unsupported/missing font role; generic font retained".into(),
                );
                None
            }
        });
    }
    if let Some(effect) = optional(node, ns, "effectRef", context)? {
        context.attributes(effect, &["idx"]);
        let index = index(effect, context);
        let placeholder = reference_color(effect, ns, theme, context)?;
        defaults.effect = Some(if index == Some(0) {
            None
        } else {
            match index
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| theme.and_then(|t| t.styles.effects.get(i as usize)))
            {
                Some(entry) => entry
                    .as_ref()
                    .and_then(|s| s.resolve(theme, placeholder, context)),
                None => {
                    context.warn(
                        "style effectRef: theme effect unavailable/out of range; effect omitted"
                            .into(),
                    );
                    None
                }
            }
        });
    }
    Ok(defaults)
}
