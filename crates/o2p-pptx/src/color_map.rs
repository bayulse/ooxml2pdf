//! Scheme-token indirection. A map never changes the theme's concrete colors.
use std::collections::BTreeMap;

use crate::{ParseLimits, PptxError, PptxWarning, ThemeColorSlot, xml};
use roxmltree::Node;

const TOKENS: [&str; 12] = [
    "bg1", "tx1", "bg2", "tx2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

/// Owned mapping from semantic scheme tokens to concrete theme slots.
/// An invalid/missing attribute in an explicit map stays unavailable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorMap {
    slots: BTreeMap<String, Option<ThemeColorSlot>>,
}

impl ColorMap {
    pub fn resolve_slot(&self, token: &str) -> Option<ThemeColorSlot> {
        self.slots
            .get(token)
            .copied()
            .unwrap_or_else(|| ThemeColorSlot::from_name(token))
    }
}

pub(crate) fn effective(
    root: Node<'_, '_>,
    master: &ColorMap,
    parent: &ColorMap,
    context: &mut crate::theme_xml::Context<'_>,
) -> Result<ColorMap, PptxError> {
    let pml = root.tag_name().namespace().unwrap_or("");
    let dml = if pml == "http://purl.oclc.org/ooxml/presentationml/main" {
        "http://purl.oclc.org/ooxml/drawingml/main"
    } else {
        "http://schemas.openxmlformats.org/drawingml/2006/main"
    };
    if root.tag_name().name() == "sldMaster" {
        return single(root, pml, "clrMap", context)?
            .map(|n| mapping(n, context))
            .transpose()
            .map(|m| m.unwrap_or_default());
    }
    let Some(node) = single(root, pml, "clrMapOvr", context)? else {
        return Ok(parent.clone());
    };
    context.attributes(node, &[]);
    context.element_only(node)?;
    let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
    if children.len() != 1 {
        context.warn(
            "malformed clrMapOvr: expected exactly one mapping; color mapping unavailable".into(),
        );
        return Ok(unavailable());
    }
    let child = children[0];
    if child.has_tag_name((dml, "masterClrMapping")) {
        context.attributes(child, &[]);
        context.leaf(child)?;
        Ok(master.clone())
    } else if child.has_tag_name((dml, "overrideClrMapping")) {
        mapping(child, context)
    } else {
        context.warn("malformed clrMapOvr: unknown mapping; color mapping unavailable".into());
        Ok(unavailable())
    }
}

fn unavailable() -> ColorMap {
    ColorMap {
        slots: TOKENS.into_iter().map(|n| (n.into(), None)).collect(),
    }
}

fn mapping(
    node: Node<'_, '_>,
    context: &mut crate::theme_xml::Context<'_>,
) -> Result<ColorMap, PptxError> {
    context.attributes(node, &TOKENS);
    context.leaf(node)?;
    let mut map = ColorMap::default();
    for token in TOKENS {
        let slot = node.attribute(token).and_then(ThemeColorSlot::from_name);
        if slot.is_none() {
            context.warn(format!("malformed color mapping {}@{token}: missing or invalid theme slot; color unavailable", node.tag_name().name()));
        }
        map.slots.insert(token.into(), slot);
    }
    Ok(map)
}

pub(crate) fn load(
    bytes: &[u8],
    part: &str,
    slide_id: u32,
    limits: ParseLimits,
    master: &ColorMap,
    parent: &ColorMap,
    warnings: &mut Vec<PptxWarning>,
) -> Result<ColorMap, PptxError> {
    xml::with_document(bytes, part, limits, |document| {
        effective(
            document.root_element(),
            master,
            parent,
            &mut crate::theme_xml::Context {
                part,
                slide_id: Some(slide_id),
                warnings,
                color_map: None,
            },
        )
    })
}

fn single<'a, 'input>(
    root: Node<'a, 'input>,
    ns: &str,
    name: &str,
    context: &crate::theme_xml::Context<'_>,
) -> Result<Option<Node<'a, 'input>>, PptxError> {
    let mut nodes = root.children().filter(|n| n.has_tag_name((ns, name)));
    let first = nodes.next();
    if nodes.next().is_some() {
        return Err(context.invalid(format!("duplicate {name}")));
    }
    Ok(first)
}
