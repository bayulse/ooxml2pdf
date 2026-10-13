//! OpenType naming adapter. A typographic family may contain several distinct
//! four-style/design families; it is not automatically the rendering identity.
use std::collections::BTreeSet;

/// One supported, decoded localized OpenType name record. Numeric IDs are
/// retained without exposing parser types. Non-Unicode encodings are omitted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontName {
    pub name_id: u16,
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub value: String,
}

#[derive(Debug)]
pub(crate) struct Names {
    pub records: Vec<FontName>,
}

impl Names {
    pub fn read(face: &ttf_parser::Face<'_>) -> Self {
        let mut records: Vec<_> = face
            .names()
            .into_iter()
            .filter_map(|n| {
                if !matches!(n.name_id, 1 | 2 | 4 | 6 | 16 | 17 | 21 | 22) || n.name.len() > 512 {
                    return None;
                }
                let value = n.to_string()?;
                let value = value.trim();
                if value.is_empty() || value.len() > 256 {
                    return None;
                }
                Some(FontName {
                    name_id: n.name_id,
                    platform_id: n.platform_id as u16,
                    encoding_id: n.encoding_id,
                    language_id: n.language_id,
                    value: value.into(),
                })
            })
            .collect();
        records.sort();
        records.dedup();
        Self { records }
    }

    pub fn preferred(&self, id: u16) -> Option<String> {
        self.records
            .iter()
            .filter(|n| n.name_id == id)
            .min_by_key(|n| {
                // Locale-independent: Windows en-US, other Windows English,
                // Unicode platform, then remaining language/platform/encoding IDs.
                let locale = match (n.platform_id, n.language_id) {
                    (3, 0x0409) => 0,
                    (3, lang) if lang & 0x03ff == 9 => 1,
                    (0, _) => 2,
                    _ => 3,
                };
                (
                    locale,
                    n.language_id,
                    n.platform_id,
                    n.encoding_id,
                    &n.value,
                )
            })
            .map(|n| n.value.clone())
    }

    pub fn family(&self) -> Option<String> {
        // WWS explicitly separates non-weight/width/slope design variants.
        // Legacy families preserve the four-style identity when WWS is absent.
        self.preferred(21)
            .or_else(|| self.preferred(1))
            .or_else(|| self.preferred(16))
    }

    pub fn subfamily(&self) -> String {
        let id = if self.preferred(21).is_some() {
            22
        } else if self.preferred(1).is_some() {
            2
        } else {
            17
        };
        self.preferred(id)
            .or_else(|| self.preferred(17))
            .or_else(|| self.preferred(2))
            .unwrap_or_default()
    }

    pub fn aliases(&self) -> Vec<String> {
        let mut aliases: Vec<_> = self
            .records
            .iter()
            .filter(|n| matches!(n.name_id, 1 | 16 | 21))
            .map(|n| n.value.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        aliases.sort_by_key(|s| (crate::registry::key(s), s.clone()));
        aliases
    }
}
