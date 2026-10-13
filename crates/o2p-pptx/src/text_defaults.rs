//! Supported sparse text properties for the nine DrawingML paragraph levels.
use crate::PlaceholderTextDefaults;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextListStyle {
    pub(crate) levels: [PlaceholderTextDefaults; 9],
}
impl TextListStyle {
    /// Zero-based paragraph level; levels outside 0..=8 are unavailable.
    pub fn level(&self, level: usize) -> Option<&PlaceholderTextDefaults> {
        self.levels.get(level)
    }
    pub(crate) fn inherit(&mut self, parent: &Self) {
        for (local, inherited) in self.levels.iter_mut().zip(&parent.levels) {
            local.inherit(inherited);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MasterTextStyles {
    pub title: TextListStyle,
    pub body: TextListStyle,
    pub other: TextListStyle,
}
impl MasterTextStyles {
    pub(crate) fn for_kind(&self, kind: &str) -> &TextListStyle {
        match kind {
            "title" | "ctrTitle" => &self.title,
            "ftr" | "sldNum" | "dt" => &self.other,
            _ => &self.body,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InsetDefaults(pub [Option<f32>; 4]);
impl InsetDefaults {
    pub fn inherit(&mut self, parent: &Self) {
        for (local, inherited) in self.0.iter_mut().zip(parent.0) {
            *local = local.or(inherited);
        }
    }
}

#[derive(Default)]
pub(crate) struct BodyDefaults {
    pub levels: TextListStyle,
    pub insets: InsetDefaults,
    pub anchor: Option<o2p_core::VerticalAlignment>,
    pub edge_spacing: Option<bool>,
}
