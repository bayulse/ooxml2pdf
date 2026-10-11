//! Private, bounded XML adapter. Only presentation size and ordered IDs acquire
//! semantics. Slide contents and unsupported presentation subtrees stay opaque.

use std::{borrow::Cow, collections::BTreeSet};

use roxmltree::{Document, Node, ParsingOptions};

use crate::{ParseLimits, PptxError, PptxWarning, SlideSize};

const PML: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const STRICT_PML: &str = "http://purl.oclc.org/ooxml/presentationml/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const STRICT_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";

pub(super) struct Metadata {
    pub size: SlideSize,
    pub slides: Vec<(u32, String)>,
    pub warnings: Vec<PptxWarning>,
}

fn invalid(part: &str, message: impl Into<String>) -> PptxError {
    PptxError::InvalidMetadata {
        part: part.into(),
        message: message.into(),
    }
}

fn xml_error(part: &str, message: impl Into<String>) -> PptxError {
    PptxError::InvalidXml {
        part: part.into(),
        message: message.into(),
    }
}

fn limit(name: &'static str) -> PptxError {
    PptxError::LimitExceeded { limit: name }
}

fn decode<'a>(bytes: &'a [u8], part: &str) -> Result<Cow<'a, str>, PptxError> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if bytes.len() % 2 != 0 {
            return Err(xml_error(part, "odd UTF-16 byte length"));
        }
        let little = bytes[0] == 0xff;
        let words = bytes[2..].chunks_exact(2).map(|b| {
            if little {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            }
        });
        let mut text = String::new();
        for c in char::decode_utf16(words) {
            text.push(c.map_err(|_| xml_error(part, "invalid UTF-16"))?);
        }
        Ok(Cow::Owned(text))
    } else {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        std::str::from_utf8(bytes)
            .map(Cow::Borrowed)
            .map_err(|_| xml_error(part, "invalid UTF-8 (UTF-16 requires BOM)"))
    }
}

fn validate_prolog(text: &str, utf16: bool, part: &str) -> Result<(), PptxError> {
    if let Some(declaration) = text
        .strip_prefix("<?xml")
        .filter(|tail| tail.starts_with([' ', '\t', '\r', '\n']))
    {
        let (attributes, _) = declaration
            .split_once("?>")
            .ok_or_else(|| xml_error(part, "unclosed XML declaration"))?;
        if attributes.contains(['&', '<']) {
            return Err(xml_error(
                part,
                "entities and markup are prohibited in XML declarations",
            ));
        }
        // roxmltree checks declaration grammar, but not version/encoding values.
        // Parse its attributes through the same private adapter, without a
        // second custom attribute/entity parser.
        let wrapper = format!("<declaration {attributes}/>");
        let declaration = Document::parse_with_options(
            &wrapper,
            ParsingOptions {
                nodes_limit: 2,
                ..ParsingOptions::default()
            },
        )
        .map_err(|error| xml_error(part, error.to_string()))?;
        let node = declaration.root_element();
        if node.attribute("version") != Some("1.0")
            || node.attribute("encoding").is_some_and(|encoding| {
                !encoding.eq_ignore_ascii_case(if utf16 { "UTF-16" } else { "UTF-8" })
            })
            || node
                .attribute("standalone")
                .is_some_and(|value| !matches!(value, "yes" | "no"))
        {
            return Err(xml_error(part, "unsupported XML declaration"));
        }
    }
    Ok(())
}

/// Bound nesting BEFORE roxmltree's recursive tree builder runs. This is only a
/// lexical resource check; roxmltree still validates all names, nesting, XML
/// grammar and namespaces. DTDs cannot inject markup via custom entities.
fn preflight(text: &str, part: &str, max_depth: usize) -> Result<(), PptxError> {
    let ceiling = max_depth.min(64);
    let mut depth = 0usize;
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let opaque = if rest.starts_with("<!--") {
            Some((4, "-->"))
        } else if rest.starts_with("<![CDATA[") {
            Some((9, "]]>"))
        } else if rest.starts_with("<?") {
            Some((2, "?>"))
        } else {
            None
        };
        if let Some((prefix, end)) = opaque {
            let tail = &rest[prefix..];
            rest = tail
                .split_once(end)
                .map(|(_, tail)| tail)
                .ok_or_else(|| xml_error(part, "unterminated XML token"))?;
            continue;
        }
        if rest.starts_with("<!") {
            return Err(xml_error(part, "DTDs and declarations are prohibited"));
        }
        let closing = rest.starts_with("</");
        if !closing {
            depth = depth.saturating_add(1);
            if depth > ceiling {
                return Err(limit("max_xml_depth"));
            }
        }
        let mut quote = None;
        let mut end = None;
        for (index, byte) in rest.bytes().enumerate().skip(1) {
            match (quote, byte) {
                (Some(q), b) if q == b => quote = None,
                (None, b'\'' | b'"') => quote = Some(byte),
                (None, b'>') => {
                    end = Some(index);
                    break;
                }
                _ => {}
            }
        }
        let end = end.ok_or_else(|| xml_error(part, "unterminated XML tag"))?;
        if closing || rest.as_bytes()[end - 1] == b'/' {
            depth = depth.saturating_sub(1);
        }
        rest = &rest[end + 1..];
    }
    Ok(())
}

fn warning(part: &str, name: String) -> PptxWarning {
    PptxWarning::UnsupportedPresentationMetadata {
        part: part.into(),
        name,
    }
}

pub(super) fn expanded(namespace: Option<&str>, local: &str) -> String {
    // Namespace bindings are shared by the XML tree. Copying a huge namespace
    // URI into every warning could amplify bounded XML into unbounded reports.
    let diagnostic = |value: &str| {
        let mut clipped: String = value.chars().take(160).collect();
        if clipped.len() < value.len() {
            clipped.push('…');
        }
        clipped
    };
    match namespace {
        Some(namespace) => format!("{{{}}}{}", diagnostic(namespace), diagnostic(local)),
        None => diagnostic(local),
    }
}

fn warn_attributes(
    node: Node<'_, '_>,
    known: &[(&str, &str)],
    part: &str,
    warnings: &mut Vec<PptxWarning>,
) {
    for attr in node.attributes() {
        if !known.contains(&(attr.namespace().unwrap_or(""), attr.name())) {
            warnings.push(warning(
                part,
                format!(
                    "{}@{}",
                    expanded(None, node.tag_name().name()),
                    expanded(attr.namespace(), attr.name())
                ),
            ));
        }
    }
}

fn element_only(node: Node<'_, '_>, part: &str) -> Result<(), PptxError> {
    if node
        .children()
        .any(|child| child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty()))
    {
        return Err(invalid(
            part,
            format!("unexpected text in {}", node.tag_name().name()),
        ));
    }
    Ok(())
}

fn leaf(node: Node<'_, '_>, part: &str) -> Result<(), PptxError> {
    element_only(node, part)?;
    if node.children().any(|child| child.is_element()) {
        return Err(invalid(
            part,
            format!("nested metadata in {}", node.tag_name().name()),
        ));
    }
    Ok(())
}

fn integer(node: Node<'_, '_>, name: &str, part: &str) -> Result<u32, PptxError> {
    let raw = node.attribute(name).ok_or_else(|| {
        invalid(
            part,
            format!("missing {name} on {}", node.tag_name().name()),
        )
    })?;
    let trimmed = raw.trim();
    let digits = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(part, format!("invalid unsigned integer {name}")));
    }
    digits
        .parse()
        .map_err(|_| invalid(part, format!("overflowing integer {name}")))
}

pub(super) fn with_document<T>(
    bytes: &[u8],
    part: &str,
    limits: ParseLimits,
    parse: impl FnOnce(&Document<'_>) -> Result<T, PptxError>,
) -> Result<T, PptxError> {
    if bytes.len() > limits.max_xml_size {
        return Err(limit("max_xml_size"));
    }
    let text = decode(bytes, part)?;
    if text.len() > limits.max_xml_size {
        return Err(limit("max_xml_size"));
    }
    preflight(&text, part, limits.max_xml_depth)?;
    let document = Document::parse_with_options(
        &text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: limits.max_xml_nodes,
            ..ParsingOptions::default()
        },
    )
    .map_err(|error| match error {
        roxmltree::Error::NodesLimitReached => limit("max_xml_nodes"),
        _ => xml_error(part, error.to_string()),
    })?;
    // Declaration grammar has now been validated, so its attribute wrapper
    // cannot contain attacker-supplied nested markup.
    validate_prolog(
        &text,
        bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]),
        part,
    )?;
    parse(&document)
}

pub(super) fn parse(bytes: &[u8], part: &str, limits: ParseLimits) -> Result<Metadata, PptxError> {
    with_document(bytes, part, limits, |document| {
        metadata(document, part, limits)
    })
}

fn metadata(
    document: &Document<'_>,
    part: &str,
    limits: ParseLimits,
) -> Result<Metadata, PptxError> {
    let root = document.root_element();
    let namespace = root.tag_name().namespace().unwrap_or("");
    if root.tag_name().name() != "presentation" || !matches!(namespace, PML | STRICT_PML) {
        return Err(invalid(part, "expected PresentationML presentation root"));
    }
    let relationship_namespace = if namespace == PML { REL } else { STRICT_REL };
    let mut size = None;
    let mut slides = None;
    let mut warnings = vec![];
    element_only(root, part)?;
    warn_attributes(root, &[], part, &mut warnings);
    for node in root.children().filter(|node| node.is_element()) {
        let tag = node.tag_name();
        match (tag.namespace(), tag.name()) {
            (Some(ns), "sldSz") if ns == namespace => {
                if size.is_some() {
                    return Err(invalid(part, "duplicate sldSz"));
                }
                leaf(node, part)?;
                let width_emu = integer(node, "cx", part)?;
                let height_emu = integer(node, "cy", part)?;
                if !(914_400..=51_206_400).contains(&width_emu)
                    || !(914_400..=51_206_400).contains(&height_emu)
                {
                    return Err(invalid(
                        part,
                        "slide size must be between 914400 and 51206400 EMUs",
                    ));
                }
                // Size type is a descriptive preset; cx/cy always define geometry.
                warn_attributes(
                    node,
                    &[("", "cx"), ("", "cy"), ("", "type")],
                    part,
                    &mut warnings,
                );
                size = Some(SlideSize {
                    width_emu,
                    height_emu,
                });
            }
            (Some(ns), "sldIdLst") if ns == namespace => {
                if slides.is_some() {
                    return Err(invalid(part, "duplicate sldIdLst"));
                }
                element_only(node, part)?;
                warn_attributes(node, &[], part, &mut warnings);
                let mut ids = BTreeSet::new();
                let mut relations = BTreeSet::new();
                let mut ordered = vec![];
                for entry in node.children().filter(|node| node.is_element()) {
                    if entry.tag_name().namespace() != Some(namespace)
                        || entry.tag_name().name() != "sldId"
                    {
                        return Err(invalid(part, "unsupported element in sldIdLst"));
                    }
                    if ordered.len() >= limits.max_slides {
                        return Err(limit("max_slides"));
                    }
                    leaf(entry, part)?;
                    let id = integer(entry, "id", part)?;
                    if !(256..=2_147_483_647).contains(&id) || !ids.insert(id) {
                        return Err(invalid(part, "invalid or duplicate slide id"));
                    }
                    let relation = entry
                        .attribute((relationship_namespace, "id"))
                        .filter(|value| !value.is_empty() && value.trim() == *value)
                        .ok_or_else(|| invalid(part, "missing or empty slide relationship id"))?;
                    if !relations.insert(relation) {
                        return Err(invalid(part, "duplicate slide relationship reference"));
                    }
                    warn_attributes(
                        entry,
                        &[("", "id"), (relationship_namespace, "id")],
                        part,
                        &mut warnings,
                    );
                    ordered.push((id, relation.into()));
                }
                slides = Some(ordered);
            }
            _ => warnings.push(warning(part, expanded(tag.namespace(), tag.name()))),
        }
    }
    let size = size.ok_or_else(|| invalid(part, "missing sldSz"))?;
    let slides = slides.ok_or_else(|| invalid(part, "missing sldIdLst"))?;
    if slides.is_empty() {
        return Err(invalid(part, "presentation has no slide references"));
    }
    Ok(Metadata {
        size,
        slides,
        warnings,
    })
}
