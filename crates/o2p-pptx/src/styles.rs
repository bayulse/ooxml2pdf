//! Owned style-matrix semantics. Index mapping belongs only to PPTX.
use crate::{Fill, Theme, inheritance::PlaceholderTextDefaults};
use o2p_core::Color;

/// Ordered theme format styles. Unsupported entries retain their index slots.
/// XML and deferred color expressions remain private implementation details.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleMatrix {
    pub(crate) fills: Vec<StyleFill>,
    pub(crate) lines: Vec<StyleLine>,
    pub(crate) backgrounds: Vec<StyleFill>,
    pub(crate) effects: Vec<Option<crate::effects::OuterShadow>>,
}

impl StyleMatrix {
    pub fn effect_style_count(&self) -> usize {
        self.effects.len()
    }
    pub fn fill_style_count(&self) -> usize {
        self.fills.len()
    }
    pub fn line_style_count(&self) -> usize {
        self.lines.len()
    }
    pub fn background_fill_style_count(&self) -> usize {
        self.backgrounds.len()
    }

    pub(crate) fn fill(&self, index: u32) -> Option<&StyleFill> {
        match index {
            0 | 1000 => Some(&StyleFill::None),
            1..=999 => self.fills.get((index - 1) as usize),
            _ => self.backgrounds.get((index - 1001) as usize),
        }
    }
    pub(crate) fn line(&self, index: u32) -> Option<&StyleLine> {
        index
            .checked_sub(1)
            .and_then(|i| self.lines.get(i as usize))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StyleFill {
    None,
    Solid(crate::theme::ColorValue),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleLine {
    pub width: Option<u32>, // EMUs; zero means no stroke.
    pub fill: StyleFill,
}

/// Resolved references stay separate from direct properties through inheritance.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StyleDefaults {
    pub fill: Option<Option<Fill>>,
    pub line_width: Option<f32>,
    pub line_color: Option<Option<Color>>,
    pub text: PlaceholderTextDefaults,
    pub effect: Option<Option<crate::effects::ResolvedShadow>>,
}
impl StyleDefaults {
    pub fn inherit(&mut self, parent: &Self) {
        self.fill = self.fill.or(parent.fill);
        self.effect = self.effect.or(parent.effect);
        self.line_width = self.line_width.or(parent.line_width);
        self.line_color = self.line_color.or(parent.line_color);
        self.text.inherit(&parent.text);
    }
}

impl StyleFill {
    pub(crate) fn resolve(
        &self,
        theme: Option<&Theme>,
        placeholder: Option<Color>,
        context: &mut crate::theme_xml::Context<'_>,
    ) -> Option<Fill> {
        match self {
            Self::None => Some(Fill::None),
            Self::Solid(color) => color.resolve(theme, placeholder, context).map(Fill::Solid),
            Self::Unsupported(name) => {
                context.warn(format!(
                    "style-matrix {name}: unsupported style; paint omitted"
                ));
                None
            }
        }
    }
}
