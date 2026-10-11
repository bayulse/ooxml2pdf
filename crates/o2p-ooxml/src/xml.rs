//! Strict metadata-only XML adapter. UTF-8/UTF-16, namespaces and predefined/numeric
//! entities are supported. DTDs and nested/unknown OPC metadata return errors.
use crate::{
    ContentType, PackageError, Relationship, RelationshipTarget, Result, SecurityLimits, limit,
    normalize_part_path, resolve_internal_target,
};
use std::collections::{BTreeMap, BTreeSet};

const TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS: &str = "http://www.w3.org/2000/xmlns/";
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}
fn xml_char(c: char) -> bool {
    matches!(c as u32, 9 | 10 | 13 | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
}
fn name(value: &str, qualified: bool) -> bool {
    let mut components = value.split(':');
    let valid = |s: &str| {
        let mut chars = s.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    };
    valid(components.next().unwrap_or(""))
        && match components.next() {
            None => true,
            Some(component) => qualified && valid(component) && components.next().is_none(),
        }
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    part: &'a str,
}
impl<'a> Parser<'a> {
    fn error(&self, message: &str) -> PackageError {
        PackageError::InvalidXml {
            part: self.part.into(),
            message: format!("{message} at byte {}", self.pos),
        }
    }
    fn tail(&self) -> &'a str {
        &self.input[self.pos..]
    }
    fn take(&mut self, token: &str) -> bool {
        if self.tail().starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }
    fn spaces(&mut self) -> bool {
        let start = self.pos;
        while let Some(c) = self.tail().chars().next().filter(|c| whitespace(*c)) {
            self.pos += c.len_utf8();
        }
        self.pos != start
    }
    fn read_name(&mut self) -> Result<String> {
        let len = self
            .tail()
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')))
            .unwrap_or(self.tail().len());
        let value = &self.tail()[..len];
        if !name(value, true) {
            return Err(self.error("Invalid or unsupported XML name"));
        }
        self.pos += len;
        Ok(value.into())
    }
    fn decode(&self, input: &str) -> Result<String> {
        let mut result = String::new();
        let mut rest = input;
        while let Some(offset) = rest.find('&') {
            result.push_str(&rest[..offset]);
            rest = &rest[offset + 1..];
            let end = rest
                .find(';')
                .ok_or_else(|| self.error("Unterminated entity"))?;
            let entity = &rest[..end];
            let c = match entity {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "apos" => '\'',
                "quot" => '"',
                _ => {
                    let code = if let Some(hex) = entity.strip_prefix("#x") {
                        if hex.is_empty() || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                            return Err(self.error("Invalid numeric entity"));
                        }
                        u32::from_str_radix(hex, 16).ok()
                    } else if let Some(decimal) = entity.strip_prefix('#') {
                        if decimal.is_empty() || !decimal.bytes().all(|c| c.is_ascii_digit()) {
                            return Err(self.error("Invalid numeric entity"));
                        }
                        decimal.parse().ok()
                    } else {
                        return Err(self.error("Undeclared entity (DTDs are prohibited)"));
                    };
                    code.and_then(char::from_u32)
                        .filter(|c| xml_char(*c))
                        .ok_or_else(|| self.error("Invalid XML character entity"))?
                }
            };
            result.push(c);
            rest = &rest[end + 1..];
        }
        result.push_str(rest);
        Ok(result)
    }
    fn declaration(&mut self, utf16: bool) -> Result<()> {
        let mut last = 0;
        loop {
            let separator = self.spaces();
            if self.take("?>") {
                return if last == 0 {
                    Err(self.error("XML version is required"))
                } else {
                    Ok(())
                };
            }
            if !separator {
                return Err(self.error("Declaration separator required"));
            }
            let key = self.read_name()?;
            let position = match key.as_str() {
                "version" => 1,
                "encoding" => 2,
                "standalone" => 3,
                _ => return Err(self.error("Unknown XML declaration attribute")),
            };
            if (last == 0 && position != 1) || position <= last {
                return Err(self.error("Invalid XML declaration order"));
            }
            self.spaces();
            if !self.take("=") {
                return Err(self.error("Declaration '=' required"));
            }
            self.spaces();
            let quote = if self.take("\"") {
                '"'
            } else if self.take("'") {
                '\''
            } else {
                return Err(self.error("Quoted declaration value required"));
            };
            let len = self
                .tail()
                .find(quote)
                .ok_or_else(|| self.error("Unclosed declaration"))?;
            let value = &self.tail()[..len];
            let valid = match key.as_str() {
                "version" => value == "1.0",
                "encoding" => value.eq_ignore_ascii_case(if utf16 { "UTF-16" } else { "UTF-8" }),
                "standalone" => value == "yes" || value == "no",
                _ => false,
            };
            if !valid {
                return Err(self.error("Invalid or unsupported XML declaration value"));
            }
            self.pos += len + 1;
            last = position;
        }
    }
    fn attributes(&mut self, end: &str) -> Result<BTreeMap<String, String>> {
        let mut attrs = BTreeMap::new();
        loop {
            let space = self.spaces();
            if self.tail().starts_with(end) || self.tail().starts_with("/>") {
                return Ok(attrs);
            }
            if !space {
                return Err(self.error("Attribute separator required"));
            }
            let key = self.read_name()?;
            self.spaces();
            if !self.take("=") {
                return Err(self.error("Attribute '=' required"));
            }
            self.spaces();
            let quote = if self.take("\"") {
                '"'
            } else if self.take("'") {
                '\''
            } else {
                return Err(self.error("Quoted attribute required"));
            };
            let len = self
                .tail()
                .find(quote)
                .ok_or_else(|| self.error("Unclosed attribute"))?;
            let raw = &self.tail()[..len];
            if raw.contains('<') {
                return Err(self.error("Raw '<' in attribute"));
            }
            // XML attribute whitespace normalization precedes entity decoding.
            let value = self.decode(&raw.replace(['\r', '\n', '\t'], " "))?;
            self.pos += len + 1;
            if attrs.insert(key, value).is_some() {
                return Err(self.error("Duplicate XML attribute"));
            }
        }
    }
    fn misc(&mut self) -> Result<()> {
        loop {
            self.spaces();
            if self.take("<!--") {
                let len = self
                    .tail()
                    .find("-->")
                    .ok_or_else(|| self.error("Unclosed comment"))?;
                let comment = &self.tail()[..len];
                if comment.contains("--") || comment.ends_with('-') {
                    return Err(self.error("Malformed comment"));
                }
                self.pos += len + 3;
            } else if self.tail().starts_with("<?") || self.tail().starts_with("<!") {
                return Err(
                    self.error("DTDs, processing instructions and declarations are unsupported")
                );
            } else {
                return Ok(());
            }
        }
    }
    fn start(&mut self, inherited: &BTreeMap<String, String>) -> Result<Node> {
        if !self.take("<") {
            return Err(self.error("Expected element"));
        }
        let qname = self.read_name()?;
        let mut attrs = self.attributes(">")?;
        let empty = self.take("/>");
        if !empty && !self.take(">") {
            return Err(self.error("Unclosed element"));
        }
        let mut namespaces = BTreeMap::from([("xml".into(), XML.into())]);
        for (key, value) in &attrs {
            if key == "xmlns" {
                if value == XML || value == XMLNS {
                    return Err(self.error("Reserved namespace"));
                }
                namespaces.insert(String::new(), value.clone());
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                if prefix == "xmlns"
                    || value == XMLNS
                    || (prefix == "xml") != (value == XML)
                    || value.is_empty()
                {
                    return Err(self.error("Invalid namespace declaration"));
                }
                namespaces.insert(prefix.into(), value.clone());
            }
        }
        attrs.retain(|key, _| key != "xmlns" && !key.starts_with("xmlns:"));
        let resolve = |qualified: &str, attribute: bool| -> Result<(String, String)> {
            if let Some((prefix, local)) = qualified.split_once(':') {
                if prefix == "xmlns" {
                    return Err(self.error("Reserved element prefix"));
                }
                let ns = namespaces
                    .get(prefix)
                    .or_else(|| inherited.get(prefix))
                    .filter(|ns| !ns.is_empty())
                    .ok_or_else(|| self.error("Unbound namespace prefix"))?;
                Ok((ns.clone(), local.into()))
            } else {
                Ok((
                    if attribute {
                        String::new()
                    } else {
                        namespaces
                            .get("")
                            .or_else(|| inherited.get(""))
                            .cloned()
                            .unwrap_or_default()
                    },
                    qualified.into(),
                ))
            }
        };
        let (namespace, local) = resolve(&qname, false)?;
        let mut expanded = BTreeSet::new();
        for key in attrs.keys() {
            let identity = resolve(key, true)?;
            if !expanded.insert(identity) {
                return Err(self.error("Duplicate expanded attribute"));
            }
            if key.contains(':') {
                return Err(self.error("Namespaced OPC attributes are unsupported"));
            }
        }
        Ok(Node {
            qname,
            local,
            namespace,
            namespaces,
            attrs,
            empty,
        })
    }
    fn close(&mut self, qname: &str) -> Result<()> {
        if !self.take("</") {
            return Err(self.error("Expected closing tag (nested metadata is unsupported)"));
        }
        if self.read_name()? != qname {
            return Err(self.error("Mismatched closing tag"));
        }
        self.spaces();
        if !self.take(">") {
            return Err(self.error("Invalid closing tag"));
        }
        Ok(())
    }
}
struct Node {
    qname: String,
    local: String,
    namespace: String,
    namespaces: BTreeMap<String, String>,
    attrs: BTreeMap<String, String>,
    empty: bool,
}

fn document(
    bytes: &[u8],
    part: &str,
    namespace: &str,
    root: &str,
    limits: SecurityLimits,
) -> Result<Vec<Node>> {
    if bytes.len() as u64 > limits.max_xml_size {
        return Err(limit("max_xml_size"));
    }
    let utf16 = bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]);
    let input = if utf16 {
        if bytes.len() % 2 != 0 {
            return Err(PackageError::InvalidXml {
                part: part.into(),
                message: "Odd UTF-16 byte length".into(),
            });
        }
        let little = bytes[0] == 0xff;
        let words: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        String::from_utf16(&words).map_err(|_| PackageError::InvalidXml {
            part: part.into(),
            message: "Invalid UTF-16".into(),
        })?
    } else {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        std::str::from_utf8(bytes)
            .map_err(|_| PackageError::InvalidXml {
                part: part.into(),
                message: "Invalid UTF-8 (UTF-16 requires BOM)".into(),
            })?
            .into()
    };
    let mut parser = Parser {
        input: &input,
        pos: 0,
        part,
    };
    if !input.chars().all(xml_char) {
        return Err(parser.error("Invalid XML character"));
    }
    if parser.take("<?xml") {
        parser.declaration(utf16)?;
    }
    parser.misc()?;
    let namespaces = BTreeMap::from([("xml".into(), XML.into())]);
    let root_node = parser.start(&namespaces)?;
    if root_node.local != root || root_node.namespace != namespace || !root_node.attrs.is_empty() {
        return Err(parser.error("Unexpected OPC root or attributes"));
    }
    let mut children = Vec::new();
    if !root_node.empty {
        loop {
            parser.misc()?;
            if parser.tail().starts_with("</") {
                parser.close(&root_node.qname)?;
                break;
            }
            let child = parser.start(&root_node.namespaces)?;
            if child.namespace != namespace {
                return Err(parser.error("Unexpected OPC namespace"));
            }
            if !child.empty {
                parser.misc()?;
                parser.close(&child.qname)?;
            }
            children.push(child);
            if root == "Relationships" && children.len() > limits.max_relationships {
                return Err(limit("max_relationships"));
            }
        }
    }
    parser.misc()?;
    if !parser.tail().is_empty() {
        return Err(parser.error("Trailing XML content"));
    }
    Ok(children)
}
fn metadata(message: impl Into<String>) -> PackageError {
    PackageError::InvalidMetadata(message.into())
}
fn required(attrs: &mut BTreeMap<String, String>, key: &str) -> Result<String> {
    attrs
        .remove(key)
        .filter(|v| !v.is_empty() && v.trim() == v)
        .ok_or_else(|| metadata(format!("Missing or empty {key}")))
}
fn no_extra(attrs: &BTreeMap<String, String>) -> Result<()> {
    if attrs.is_empty() {
        Ok(())
    } else {
        Err(metadata(format!(
            "Unsupported metadata attributes: {:?}",
            attrs.keys().collect::<Vec<_>>()
        )))
    }
}

pub(crate) fn content_types(bytes: &[u8], limits: SecurityLimits) -> Result<Vec<ContentType>> {
    let nodes = document(bytes, "[Content_Types].xml", TYPES, "Types", limits)?;
    let mut defaults = BTreeSet::new();
    let mut overrides = BTreeSet::new();
    let mut types = Vec::new();
    for mut node in nodes {
        let mime_type = required(&mut node.attrs, "ContentType")?;
        if !mime_type.contains('/') || mime_type.chars().any(whitespace) {
            return Err(metadata("Invalid MIME content type"));
        }
        let declaration = match node.local.as_str() {
            "Default" => {
                let extension = required(&mut node.attrs, "Extension")?.to_ascii_lowercase();
                if !extension
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                    || !defaults.insert(extension.clone())
                {
                    return Err(metadata("Invalid or duplicate default extension"));
                }
                ContentType::Default {
                    extension,
                    mime_type,
                }
            }
            "Override" => {
                let raw = required(&mut node.attrs, "PartName")?;
                // OPC PartName uses a leading '/', unlike ZIP names or relationship targets.
                let part_name = normalize_part_path(
                    raw.strip_prefix('/')
                        .ok_or_else(|| metadata("Override PartName must start with '/'"))?,
                )?;
                if part_name.len() > limits.max_path_length {
                    return Err(limit("max_path_length"));
                }
                if !overrides.insert(part_name.clone()) {
                    return Err(metadata("Duplicate content type override"));
                }
                ContentType::Override {
                    part_name,
                    mime_type,
                }
            }
            _ => {
                return Err(metadata(format!(
                    "Unsupported content type element {}",
                    node.local
                )));
            }
        };
        no_extra(&node.attrs)?;
        types.push(declaration);
    }
    Ok(types)
}

pub(crate) fn relationships(
    bytes: &[u8],
    part: &str,
    source: &str,
    limits: SecurityLimits,
) -> Result<Vec<Relationship>> {
    let nodes = document(bytes, part, RELS, "Relationships", limits)?;
    let mut ids = BTreeSet::new();
    let mut relations = Vec::new();
    for mut node in nodes {
        if node.local != "Relationship" {
            return Err(metadata(format!(
                "Unsupported relationship element {}",
                node.local
            )));
        }
        let id = required(&mut node.attrs, "Id")?;
        if !name(&id, false) || !ids.insert(id.clone()) {
            return Err(metadata(format!(
                "Invalid or duplicate relationship Id {id}"
            )));
        }
        let relationship_type = required(&mut node.attrs, "Type")?;
        let valid_type = relationship_type
            .split_once(':')
            .is_some_and(|(scheme, value)| {
                !scheme.is_empty()
                    && !value.is_empty()
                    && scheme.as_bytes()[0].is_ascii_alphabetic()
                    && scheme
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
            });
        if !valid_type || relationship_type.chars().any(whitespace) {
            return Err(metadata("Relationship Type must be an absolute URI"));
        }
        let raw = required(&mut node.attrs, "Target")?;
        if raw.chars().any(char::is_control) {
            return Err(metadata("Relationship Target contains control characters"));
        }
        let target = match node.attrs.remove("TargetMode").as_deref() {
            None | Some("Internal") => RelationshipTarget::Internal(resolve_internal_target(
                if source.is_empty() {
                    None
                } else {
                    Some(source)
                },
                &raw,
            )?),
            Some("External") => RelationshipTarget::External(raw),
            Some(_) => return Err(metadata("Invalid relationship TargetMode")),
        };
        if matches!(&target, RelationshipTarget::Internal(name) if name.len() > limits.max_path_length)
        {
            return Err(limit("max_path_length"));
        }
        no_extra(&node.attrs)?;
        relations.push(Relationship {
            id,
            relationship_type,
            target,
        });
    }
    Ok(relations)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn types(inner: &str) -> String {
        format!("<Types xmlns='{TYPES}'>{inner}</Types>")
    }
    fn rels(inner: &str) -> String {
        format!("<Relationships xmlns='{RELS}'>{inner}</Relationships>")
    }
    #[test]
    fn parses_content_types_and_entities() {
        let parsed = content_types(types("<Default Extension='XML' ContentType='application/xml'/><Override PartName='/ppt/slide.xml' ContentType='application/s&#108;ide'/>").as_bytes(), SecurityLimits::default()).unwrap();
        assert_eq!(
            parsed[0],
            ContentType::Default {
                extension: "xml".into(),
                mime_type: "application/xml".into()
            }
        );
        assert_eq!(
            parsed[1],
            ContentType::Override {
                part_name: "ppt/slide.xml".into(),
                mime_type: "application/slide".into()
            }
        );
    }
    #[test]
    fn namespaces_comments_and_utf16() {
        let input = format!(
            "<?xml version='1.0' encoding='UTF-16'?><!-- test --><ct:Types xmlns:ct='{TYPES}'><ct:Default Extension='xml' ContentType='application/xml'></ct:Default></ct:Types>"
        );
        let bytes: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(input.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(
            content_types(&bytes, SecurityLimits::default())
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn parses_internal_and_external_relationships() {
        let bytes = rels(
            "<Relationship Id='r1' Type='urn:slide' Target='../slides/./slide1.xml'/><Relationship Id='r2' Type='urn:link' TargetMode='External' Target='https://example.invalid/?a=1&amp;b=2'/>",
        );
        let parsed = relationships(
            bytes.as_bytes(),
            "ppt/_rels/presentation.xml.rels",
            "ppt/presentation.xml",
            SecurityLimits::default(),
        )
        .unwrap();
        assert_eq!(
            parsed[0].target,
            RelationshipTarget::Internal("slides/slide1.xml".into())
        );
        assert_eq!(
            parsed[1].target,
            RelationshipTarget::External("https://example.invalid/?a=1&b=2".into())
        );
    }
    #[test]
    fn rejects_malformed_or_unsupported_xml() {
        for xml in [
            "",
            "<Types>",
            "<Types xmlns='bad'/>",
            "<!DOCTYPE Types [<!ENTITY x 'boom'>]><Types/>",
            "<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'><Default Extension='xml' ContentType='application/&unknown;'/></Types>",
            "<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'></Wrong>",
            "<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types' xmlns='duplicate'/>",
            "<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'/>trailing",
            "<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'><!-- a--b --></Types>",
            "<?xml version='1.1'?><Types/>",
        ] {
            assert!(
                content_types(xml.as_bytes(), SecurityLimits::default()).is_err(),
                "{xml}"
            );
        }
        assert!(content_types(&[0xff, 0xfe, 0], SecurityLimits::default()).is_err());
        assert!(content_types(&[0xff], SecurityLimits::default()).is_err());
    }
    #[test]
    fn rejects_invalid_metadata() {
        for inner in [
            "<Default Extension='xml' ContentType='application/xml'/><Default Extension='XML' ContentType='application/xml'/>",
            "<Override PartName='../escape' ContentType='application/xml'/>",
            "<Override PartName='/../escape' ContentType='application/xml'/>",
            "<Unknown ContentType='application/xml'/>",
        ] {
            assert!(content_types(types(inner).as_bytes(), SecurityLimits::default()).is_err());
        }
        for inner in [
            "<Relationship Id='r1' Type='urn:test'/>",
            "<Relationship Id='r1' Type='relative' Target='a'/>",
            "<Relationship Id='r1' Type='urn:test' Target='a' TargetMode='bad'/>",
            "<Relationship Id='r1' Type='urn:test' Target='a'/><Relationship Id='r1' Type='urn:test' Target='b'/>",
            "<Relationship Id='r1' Type='urn:test' Target='../escape'/>",
        ] {
            assert!(
                relationships(
                    rels(inner).as_bytes(),
                    "_rels/.rels",
                    "",
                    SecurityLimits::default()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn declaration_grammar_and_namespace_bindings_are_validated() {
        for declaration in [
            "<?xml encoding='UTF-8' version='1.0'?>",
            "<?xml version='1&#46;0'?>",
            "<?xml version='1.0' standalone='yes' encoding='UTF-8'?>",
            "<?xml version='1.0' version='1.0'?>",
            "<?xml?>",
        ] {
            assert!(
                content_types(
                    format!("{declaration}{}", types("")).as_bytes(),
                    SecurityLimits::default()
                )
                .is_err()
            );
        }
        for inner in [
            "<x:Default Extension='xml' ContentType='application/xml'/>",
            "<Default xmlns='urn:wrong' Extension='xml' ContentType='application/xml'/>",
            "<Default xmlns:xml='urn:wrong' Extension='xml' ContentType='application/xml'/>",
            "<Default Extension='xml' ContentType='application/xml'><Nested/></Default>",
            "<Default Extension='xml' ContentType='application/&#0;'/>",
        ] {
            assert!(content_types(types(inner).as_bytes(), SecurityLimits::default()).is_err());
        }
        let valid = format!(
            "<?xml version='1.0' encoding='UTF-8' standalone='yes'?>{}",
            types("")
        );
        assert!(content_types(valid.as_bytes(), SecurityLimits::default()).is_ok());
        let utf16 = types("<Default Extension='xml' ContentType='application/xml'/>");
        let bytes: Vec<u8> = [0xfe, 0xff]
            .into_iter()
            .chain(utf16.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert!(content_types(&bytes, SecurityLimits::default()).is_ok());
    }

    #[test]
    fn normalized_names_cannot_amplify_relationship_memory() {
        let limits = SecurityLimits {
            max_path_length: 20,
            ..SecurityLimits::default()
        };
        let input = rels("<Relationship Id='r1' Type='urn:test' Target='abcdefgh.dat'/>");
        assert_eq!(
            relationships(
                input.as_bytes(),
                "a/_rels/document.xml.rels",
                "long-directory/document.xml",
                limits
            )
            .unwrap_err(),
            limit("max_path_length")
        );
        let input = types(
            "<Override PartName='/long-directory/abcdefgh.dat' ContentType='application/xml'/>",
        );
        assert_eq!(
            content_types(input.as_bytes(), limits).unwrap_err(),
            limit("max_path_length")
        );
    }

    #[test]
    fn xml_and_relationship_limits() {
        assert!(matches!(
            content_types(
                types("").as_bytes(),
                SecurityLimits {
                    max_xml_size: 1,
                    ..SecurityLimits::default()
                }
            ),
            Err(PackageError::LimitExceeded {
                limit: "max_xml_size"
            })
        ));
        assert!(matches!(
            relationships(
                rels("<Relationship Id='r1' Type='urn:test' Target='a'/>").as_bytes(),
                "_rels/.rels",
                "",
                SecurityLimits {
                    max_relationships: 0,
                    ..SecurityLimits::default()
                }
            ),
            Err(PackageError::LimitExceeded {
                limit: "max_relationships"
            })
        ));
    }
}
