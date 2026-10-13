//! Owned DrawingML theme semantics. No XML, OPC or PDF implementation types.
use std::collections::BTreeMap;

use o2p_core::Color;

/// Standard concrete color-scheme slots (color-map aliases are not slots).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThemeColorSlot {
    Dark1,
    Light1,
    Dark2,
    Light2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hyperlink,
    FollowedHyperlink,
}

impl ThemeColorSlot {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "dk1" => Self::Dark1,
            "lt1" => Self::Light1,
            "dk2" => Self::Dark2,
            "lt2" => Self::Light2,
            "accent1" => Self::Accent1,
            "accent2" => Self::Accent2,
            "accent3" => Self::Accent3,
            "accent4" => Self::Accent4,
            "accent5" => Self::Accent5,
            "accent6" => Self::Accent6,
            "hlink" => Self::Hyperlink,
            "folHlink" => Self::FollowedHyperlink,
            _ => return None,
        })
    }
}

/// Only successfully resolved concrete definitions are present.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColorScheme {
    pub colors: BTreeMap<ThemeColorSlot, Color>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThemeFonts {
    pub latin: Option<String>,
    /// Preserved for future script selection; not used to infer run properties.
    pub east_asian: Option<String>,
    pub complex_script: Option<String>,
    pub scripts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontScheme {
    pub major: ThemeFonts,
    pub minor: ThemeFonts,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Theme {
    pub colors: ColorScheme,
    pub fonts: FontScheme,
    pub styles: crate::StyleMatrix,
}

/// Deferred colors allow scheme lookup and phClr substitution at the reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColorValue {
    pub base: ColorBase,
    pub transforms: Vec<(ColorTransformKind, i32)>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColorBase {
    Rgb(Color),
    Scheme(String),
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorTransformKind {
    Tint,
    Shade,
    LumMod,
    LumOff,
}

impl ColorValue {
    pub(crate) fn resolve(
        &self,
        theme: Option<&Theme>,
        placeholder: Option<Color>,
        context: &mut crate::theme_xml::Context<'_>,
    ) -> Option<Color> {
        let base = match &self.base {
            ColorBase::Rgb(color) => Some(*color),
            ColorBase::Unavailable => None,
            ColorBase::Scheme(name) => {
                let color = if name == "phClr" {
                    placeholder
                } else {
                    theme.and_then(|theme| {
                        let slot = context.color_map.map_or_else(
                            || ThemeColorSlot::from_name(name),
                            |map| map.resolve_slot(name),
                        )?;
                        theme.colors.colors.get(&slot).copied()
                    })
                };
                if color.is_none() {
                    context.warn(format!("schemeClr {}: theme color unavailable; paint omitted or paragraph/default text color retained", crate::xml::expanded(None, name)));
                }
                color
            }
        }?;
        let transforms: Vec<_> = self
            .transforms
            .iter()
            .map(|&(kind, value)| {
                let value = f64::from(value) / 100_000.0;
                match kind {
                    ColorTransformKind::Tint => ColorTransform::Tint(value),
                    ColorTransformKind::Shade => ColorTransform::Shade(value),
                    ColorTransformKind::LumMod => ColorTransform::LumMod(value),
                    ColorTransformKind::LumOff => ColorTransform::LumOff(value),
                }
            })
            .collect();
        Some(transform_color(base, &transforms))
    }
}

impl Theme {
    /// Resolve concrete scheme slots only; bg/tx/ph aliases require later semantics.
    pub fn resolve_color(&self, name: &str) -> Option<Color> {
        self.colors
            .colors
            .get(&ThemeColorSlot::from_name(name)?)
            .copied()
    }

    /// Explicit Latin theme aliases, never a master/layout font inheritance rule.
    pub fn resolve_font(&self, token: &str) -> Option<&str> {
        match token {
            "+mj-lt" => self.fonts.major.latin.as_deref(),
            "+mn-lt" => self.fonts.minor.latin.as_deref(),
            _ => None,
        }
    }
}

/// The conservative supported transform subset, with normalized percentages.
/// Values are validated by the XML adapter before reaching this module.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ColorTransform {
    Tint(f64),
    Shade(f64),
    LumMod(f64),
    LumOff(f64),
}

/// Apply in declaration order, clamp after each operation and round once at end.
/// Tint/shade mix sRGB channels; luminance transforms preserve HSL hue/saturation.
pub(crate) fn transform_color(color: Color, transforms: &[ColorTransform]) -> Color {
    let mut rgb = [color.red, color.green, color.blue].map(|v| f64::from(v) / 255.0);
    let mut hsl: Option<([f64; 3], f64, f64)> = None;
    for transform in transforms {
        match *transform {
            ColorTransform::Tint(v) => {
                rgb = rgb.map(|c| c * v + 1.0 - v);
                hsl = None;
            }
            ColorTransform::Shade(v) => {
                rgb = rgb.map(|c| c * v);
                hsl = None;
            }
            ColorTransform::LumMod(v) | ColorTransform::LumOff(v) => {
                // Retain hue/saturation at white/black between luminance
                // operations; converting back from achromatic RGB loses hue.
                let (hue, saturation, light) = hsl.unwrap_or_else(|| {
                    let max = rgb.into_iter().fold(0.0, f64::max);
                    let min = rgb.into_iter().fold(1.0, f64::min);
                    let light = (max + min) / 2.0;
                    let chroma = max - min;
                    let saturation = if chroma == 0.0 {
                        0.0
                    } else {
                        chroma / (1.0 - (2.0 * light - 1.0).abs())
                    };
                    let hue = rgb.map(|c| {
                        if chroma == 0.0 {
                            0.0
                        } else {
                            (c - min) / chroma
                        }
                    });
                    (hue, saturation, light)
                });
                let target = match transform {
                    ColorTransform::LumMod(_) => light * v,
                    _ => light + v,
                }
                .clamp(0.0, 1.0);
                let target_chroma = (1.0 - (2.0 * target - 1.0).abs()) * saturation;
                // Relative channel positions encode hue without angle conversion.
                rgb = hue.map(|c| c * target_chroma + target - target_chroma / 2.0);
                hsl = Some((hue, saturation, target));
            }
        }
        rgb = rgb.map(|c| c.clamp(0.0, 1.0));
    }
    // Prevent floating-point cancellation from rounding exact half-byte ties
    // downward; this epsilon is far below a meaningful channel difference.
    let [r, g, b] = rgb.map(|c| (c * 255.0 + 1e-10).round() as u8);
    Color::rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_and_shade_endpoints_and_midpoints() {
        let base = Color::rgb(100, 150, 200);
        assert_eq!(transform_color(base, &[]), base);
        assert_eq!(transform_color(base, &[ColorTransform::Tint(1.0)]), base);
        assert_eq!(
            transform_color(base, &[ColorTransform::Tint(0.0)]),
            Color::rgb(255, 255, 255)
        );
        assert_eq!(
            transform_color(base, &[ColorTransform::Tint(0.5)]),
            Color::rgb(178, 203, 228)
        );
        assert_eq!(transform_color(base, &[ColorTransform::Shade(1.0)]), base);
        assert_eq!(
            transform_color(base, &[ColorTransform::Shade(0.0)]),
            Color::BLACK
        );
        assert_eq!(
            transform_color(base, &[ColorTransform::Shade(0.5)]),
            Color::rgb(50, 75, 100)
        );
    }

    #[test]
    fn luminance_preserves_hue_and_saturation_with_clamping() {
        let red = Color::rgb(255, 0, 0);
        assert_eq!(
            transform_color(red, &[ColorTransform::LumMod(0.5)]),
            Color::rgb(128, 0, 0)
        );
        assert_eq!(
            transform_color(red, &[ColorTransform::LumOff(0.25)]),
            Color::rgb(255, 128, 128)
        );
        assert_eq!(
            transform_color(
                red,
                &[ColorTransform::LumMod(0.5), ColorTransform::LumOff(0.5)]
            ),
            Color::rgb(255, 128, 128)
        );
        assert_eq!(
            transform_color(
                red,
                &[ColorTransform::LumOff(0.5), ColorTransform::LumMod(0.5)]
            ),
            red
        );
        assert_eq!(
            transform_color(red, &[ColorTransform::LumMod(3.0)]),
            Color::rgb(255, 255, 255)
        );
        assert_eq!(
            transform_color(red, &[ColorTransform::LumOff(-1.0)]),
            Color::BLACK
        );
        assert_eq!(
            transform_color(Color::BLACK, &[ColorTransform::LumOff(0.5)]),
            Color::rgb(128, 128, 128)
        );
        for color in [
            Color::BLACK,
            Color::rgb(255, 255, 255),
            Color::rgb(30, 70, 160),
            Color::rgb(128, 128, 128),
        ] {
            assert_eq!(
                transform_color(
                    color,
                    &[ColorTransform::LumMod(1.0), ColorTransform::LumOff(0.0)]
                ),
                color
            );
        }
    }
}
