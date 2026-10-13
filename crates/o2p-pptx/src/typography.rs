//! DrawingML paragraph properties; sparse presence survives ancestor resolution.
use super::{Node, Parser, PlaceholderTextDefaults, PptxError, TextAlignment, emu_to_points};
use o2p_core::{ParagraphSpacing, VerticalAlignment};

impl Parser<'_> {
    pub(super) fn paragraph_defaults(
        &mut self,
        ppr: Node<'_, '_>,
    ) -> Result<PlaceholderTextDefaults, PptxError> {
        let dml = self.dml;
        self.attributes(ppr, &["algn", "lvl", "marL", "marR", "indent"]);
        self.element_only(ppr)?;
        let mut defaults = if let Some(style) = self.single(ppr, dml, "defRPr")? {
            self.run_defaults(style)?
        } else {
            PlaceholderTextDefaults::default()
        };
        if let Some(value) = ppr.attribute("algn") {
            defaults.alignment = Some(match value {
                "l" => TextAlignment::Left,
                "ctr" => TextAlignment::Center,
                "r" => TextAlignment::Right,
                _ => {
                    self.warn("unsupported paragraph alignment: using left fallback".into());
                    TextAlignment::Left
                }
            });
        }
        for (name, target) in [
            ("marL", &mut defaults.margin_left),
            ("marR", &mut defaults.margin_right),
            ("indent", &mut defaults.indent),
        ] {
            if ppr.attribute(name).is_some() {
                let minimum = if name == "indent" { -51_206_400 } else { 0 };
                *target = Some(emu_to_points(self.integer(ppr, name, minimum, 51_206_400)?));
            }
        }
        let mut bullet_count = 0;
        let mut font_count = 0;
        for child in ppr.children().filter(|n| n.is_element()) {
            if child.tag_name().namespace() != Some(dml) {
                self.unsupported(child);
                continue;
            }
            match child.tag_name().name() {
                "defRPr" => {}
                "buChar" => {
                    bullet_count += 1;
                    self.attributes(child, &["char"]);
                    self.leaf(child)?;
                    let glyph = child
                        .attribute("char")
                        .ok_or_else(|| self.invalid("buChar missing char"))?;
                    if glyph.is_empty() || glyph.len() > 256 || glyph.chars().any(char::is_control)
                    {
                        return Err(
                            self.invalid("buChar must contain a bounded Unicode bullet string")
                        );
                    }
                    defaults.bullet_char = Some(Some(glyph.into()));
                }
                "buNone" => {
                    bullet_count += 1;
                    self.attributes(child, &[]);
                    self.leaf(child)?;
                    defaults.bullet_char = Some(None);
                }
                "buAutoNum" | "buBlip" => {
                    bullet_count += 1;
                    self.unsupported(child);
                    defaults.bullet_char = Some(None);
                }
                "buFont" => {
                    font_count += 1;
                    self.attributes(child, &["typeface"]);
                    self.leaf(child)?;
                    let family = child
                        .attribute("typeface")
                        .ok_or_else(|| self.invalid("buFont missing typeface"))?;
                    if family.is_empty()
                        || family.len() > 256
                        || family.chars().any(char::is_control)
                    {
                        return Err(self.invalid("invalid bullet font family"));
                    }
                    defaults.bullet_family = Some(crate::theme_xml::font(
                        family,
                        self.theme,
                        &mut crate::theme_xml::Context {
                            color_map: Some(&self.color_map),
                            part: self.reference.part_name(),
                            slide_id: Some(self.reference.id()),
                            warnings: self.warnings,
                        },
                    ));
                }
                "buFontTx" => {
                    font_count += 1;
                    self.attributes(child, &[]);
                    self.leaf(child)?;
                    defaults.bullet_family = Some(None);
                }
                "spcBef" => defaults.space_before = Some(self.paragraph_spacing(child)?),
                "spcAft" => defaults.space_after = Some(self.paragraph_spacing(child)?),
                _ => self.unsupported(child),
            }
        }
        if bullet_count > 1 || font_count > 1 {
            return Err(self.invalid("conflicting paragraph bullet properties"));
        }
        // Enforce one spacing container, even when its form is unsupported.
        self.single(ppr, dml, "spcBef")?;
        self.single(ppr, dml, "spcAft")?;
        Ok(defaults)
    }

    fn paragraph_spacing(&mut self, node: Node<'_, '_>) -> Result<ParagraphSpacing, PptxError> {
        self.attributes(node, &[]);
        self.element_only(node)?;
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 1 {
            return Err(self.invalid("paragraph spacing requires one value"));
        }
        let value = children[0];
        self.attributes(value, &["val"]);
        self.leaf(value)?;
        if value.has_tag_name((self.dml, "spcPts")) {
            Ok(ParagraphSpacing::Points(
                self.integer(value, "val", 0, 158_400)? as f32 / 100.0,
            ))
        } else if value.has_tag_name((self.dml, "spcPct")) {
            // Transitional files use thousandths of a percent; Strict also permits
            // a decimal percent string. Normalize both to a text-size multiple.
            let raw = value
                .attribute("val")
                .ok_or_else(|| self.invalid("spacing missing val"))?;
            let multiple = if let Some(percent) = raw.strip_suffix('%') {
                percent
                    .parse::<f64>()
                    .map_err(|_| self.invalid("invalid spacing percentage"))?
                    / 100.0
            } else {
                self.integer(value, "val", 0, 13_200_000)? as f64 / 100_000.0
            };
            if !multiple.is_finite() || !(0.0..=132.0).contains(&multiple) {
                return Err(self.invalid("spacing percentage outside supported OOXML range"));
            }
            Ok(ParagraphSpacing::TextMultiple(multiple as f32))
        } else {
            self.warn(format!(
                "unsupported paragraph spacing {}: using zero spacing",
                crate::xml::expanded(value.tag_name().namespace(), value.tag_name().name())
            ));
            Ok(ParagraphSpacing::default())
        }
    }

    pub(super) fn body_anchor(
        &mut self,
        node: Node<'_, '_>,
    ) -> Result<Option<VerticalAlignment>, PptxError> {
        Ok(node.attribute("anchor").map(|anchor| match anchor {
            "t" => VerticalAlignment::Top,
            "ctr" => VerticalAlignment::Center,
            "b" => VerticalAlignment::Bottom,
            _ => {
                self.warn(format!(
                    "unsupported text vertical anchor {anchor}: using top alignment"
                ));
                VerticalAlignment::Top
            }
        }))
    }
}
