//! Bounded DrawingML text parsing, using the slide adapter's validation helpers.
use super::{
    Fill, Node, Paragraph, Parser, PptxError, Rect, TextAlignment, TextBox, TextInsets, TextRun,
    TextStyle, emu_to_points,
};

impl Parser<'_> {
    pub(super) fn text_box(
        &mut self,
        body: Node<'_, '_>,
        rect: Rect,
        inherited: Option<&crate::inheritance::Properties>,
    ) -> Result<Option<TextBox>, PptxError> {
        let dml = self.dml;
        self.attributes(body, &[]);
        self.element_only(body)?;
        self.other_children(body, &[(dml, "bodyPr"), (dml, "lstStyle"), (dml, "p")]);
        let mut insets = TextInsets::default();
        if let Some(parent) = inherited {
            for (target, value) in [
                &mut insets.left,
                &mut insets.right,
                &mut insets.top,
                &mut insets.bottom,
            ]
            .into_iter()
            .zip(parent.insets.0)
            {
                if let Some(value) = value {
                    *target = value;
                }
            }
        }
        let mut supported = true;
        if let Some(properties) = self.single(body, dml, "bodyPr")? {
            self.attributes(
                properties,
                &[
                    "lIns", "rIns", "tIns", "bIns", "anchor", "vert", "rot", "wrap",
                ],
            );
            for (name, target) in [
                ("lIns", &mut insets.left),
                ("rIns", &mut insets.right),
                ("tIns", &mut insets.top),
                ("bIns", &mut insets.bottom),
            ] {
                if properties.attribute(name).is_some() {
                    *target =
                        emu_to_points(self.integer(properties, name, 0, i64::from(i32::MAX))?);
                }
            }
            if properties.attribute("anchor").is_some_and(|v| v != "t") {
                self.warn("text vertical anchor: using top alignment".into());
            }
            if properties.attribute("vert").is_some_and(|v| v != "horz") {
                self.warn("vertical text: text omitted".into());
                supported = false;
            }
            if properties.attribute("rot").is_some()
                && self.integer(properties, "rot", i64::from(i32::MIN), i64::from(i32::MAX))? != 0
            {
                self.warn("text rotation: text omitted".into());
                supported = false;
            }
            match properties.attribute("wrap") {
                None | Some("square") => {}
                Some("none") => self.warn("text wrapping disabled: configured-font layout uses wrapping; legacy layout uses explicit breaks only".into()),
                Some(_) => self.warn("unsupported text wrapping mode: configured-font layout uses whitespace wrapping".into()),
            }
            self.element_only(properties)?;
            for child in properties.children().filter(|n| n.is_element()) {
                if child.has_tag_name((dml, "noAutofit")) {
                    self.attributes(child, &[]);
                    self.leaf(child)?;
                } else {
                    self.unsupported(child);
                }
            }
        } else {
            self.warn("txBody missing bodyPr: using default text insets".into());
        }
        let (mut levels, _) = self.body_properties(body, false)?;
        if let Some(parent) = inherited {
            levels.inherit(&parent.text_levels);
        }
        if inherited.is_none() {
            levels.inherit(&self.text_styles.other);
        }
        let mut paragraphs = vec![];
        let mut fallback = 0;
        for paragraph in body.children().filter(|n| n.has_tag_name((dml, "p"))) {
            let level = self.paragraph_level(self.single(paragraph, dml, "pPr")?)?;
            let (parsed, unresolved) = self.paragraph(paragraph, &levels.levels[level])?;
            paragraphs.push(parsed);
            fallback |= unresolved;
        }
        if fallback != 0 {
            let missing: Vec<_> = [(1, "font family"), (2, "font size"), (4, "text color")]
                .into_iter()
                .filter_map(|(bit, name)| (fallback & bit != 0).then_some(name))
                .collect();
            self.warn(format!("text inheritance/font resolution: unresolved {} after local/layout/master/theme styles; fallback is 18pt regular black sans-serif for missing properties; schema defaults are regular, left/top with 7.2pt horizontal and 3.6pt vertical insets", missing.join(", ")));
        }
        if paragraphs.is_empty() {
            return Err(self.invalid("txBody must contain a paragraph"));
        }
        Ok(supported.then_some(TextBox {
            rect,
            insets,
            paragraphs,
        }))
    }

    fn paragraph(
        &mut self,
        paragraph: Node<'_, '_>,
        inherited: &super::PlaceholderTextDefaults,
    ) -> Result<(Paragraph, u8), PptxError> {
        let dml = self.dml;
        self.attributes(paragraph, &[]);
        self.element_only(paragraph)?;
        let properties = self.single(paragraph, dml, "pPr")?;
        let end = self.single(paragraph, dml, "endParaRPr")?;
        let mut defaults = if let Some(properties) = properties {
            self.paragraph_defaults(properties)?
        } else {
            super::PlaceholderTextDefaults::default()
        };
        defaults.inherit(inherited);
        let default = defaults.style();
        let alignment = defaults.alignment.unwrap_or(TextAlignment::Left);
        let mut fallback = 0;
        let end_style = if let Some(end) = end {
            self.text_style(end, default.clone())?
        } else {
            default.clone()
        };
        let mut runs = vec![];
        let mut seen_run = false;
        let mut seen_end = false;
        for child in paragraph.children().filter(|n| n.is_element()) {
            if child.has_tag_name((dml, "pPr")) {
                if seen_run || seen_end {
                    return Err(self.invalid("pPr must precede paragraph runs"));
                }
                continue;
            }
            if child.has_tag_name((dml, "endParaRPr")) {
                seen_end = true;
                continue;
            }
            if seen_end {
                return Err(self.invalid("endParaRPr must end the paragraph"));
            }
            seen_run = true;
            if child.has_tag_name((dml, "r")) {
                self.attributes(child, &[]);
                self.other_children(child, &[(dml, "rPr"), (dml, "t")]);
                let mut effective = if let Some(style) = self.single(child, dml, "rPr")? {
                    self.run_defaults(style)?
                } else {
                    super::PlaceholderTextDefaults::default()
                };
                effective.inherit(&defaults);
                fallback |= Self::unresolved_text(&effective);
                let mut style = effective.style();
                if effective.color == Some(None) {
                    style.color = default.color;
                }
                let text = self
                    .single(child, dml, "t")?
                    .ok_or_else(|| self.invalid("text run missing a:t"))?;
                for attr in text.attributes() {
                    if !(attr.namespace() == Some("http://www.w3.org/XML/1998/namespace")
                        && attr.name() == "space")
                    {
                        self.warn(format!(
                            "text attribute {}",
                            crate::xml::expanded(attr.namespace(), attr.name())
                        ));
                    }
                }
                if text.children().any(|n| n.is_element()) {
                    return Err(self.invalid("a:t must contain literal text only"));
                }
                let literal: String = text
                    .children()
                    .filter_map(|n| if n.is_text() { n.text() } else { None })
                    .collect();
                if literal.chars().any(|c| c.is_control() && c != '\n') {
                    // XML permits tabs, but positioned runs have no tab-stop meaning.
                    return Err(self.invalid("unsupported text control character; only explicit line breaks are supported"));
                }
                runs.push(TextRun {
                    text: literal,
                    style,
                });
            } else if child.has_tag_name((dml, "br")) {
                self.attributes(child, &[]);
                self.other_children(child, &[(dml, "rPr")]);
                let mut effective = if let Some(style) = self.single(child, dml, "rPr")? {
                    self.run_defaults(style)?
                } else {
                    super::PlaceholderTextDefaults::default()
                };
                effective.inherit(&defaults);
                fallback |= Self::unresolved_text(&effective);
                let mut style = effective.style();
                if effective.color == Some(None) {
                    style.color = default.color;
                }
                runs.push(TextRun {
                    text: "\n".into(),
                    style,
                });
            } else {
                self.unsupported(child);
            }
        }
        if runs.is_empty() {
            let mut effective = if let Some(end) = end {
                self.run_defaults(end)?
            } else {
                super::PlaceholderTextDefaults::default()
            };
            effective.inherit(&defaults);
            fallback |= Self::unresolved_text(&effective);
        }
        Ok((
            Paragraph {
                runs,
                alignment,
                end_style,
            },
            fallback,
        ))
    }

    pub(super) fn paragraph_level(&self, ppr: Option<Node<'_, '_>>) -> Result<usize, PptxError> {
        ppr.filter(|n| n.attribute("lvl").is_some())
            .map(|n| self.integer(n, "lvl", 0, 8).map(|v| v as usize))
            .transpose()
            .map(|v| v.unwrap_or(0))
    }

    fn unresolved_text(defaults: &super::PlaceholderTextDefaults) -> u8 {
        u8::from(defaults.family.as_ref().is_none_or(|f| f.is_none()))
            | (u8::from(defaults.font_size.is_none()) << 1)
            | (u8::from(defaults.color.is_none_or(|c| c.is_none())) << 2)
    }

    pub(super) fn run_defaults(
        &mut self,
        node: Node<'_, '_>,
    ) -> Result<super::PlaceholderTextDefaults, PptxError> {
        let dml = self.dml;
        self.attributes(
            node,
            &["sz", "b", "i", "lang", "altLang", "dirty", "smtClean"],
        );
        self.element_only(node)?;
        let mut defaults = super::PlaceholderTextDefaults {
            font_size: if node.attribute("sz").is_some() {
                Some(self.integer(node, "sz", 100, 400_000)? as f32 / 100.0)
            } else {
                None
            },
            bold: self.boolean(node, "b")?,
            italic: self.boolean(node, "i")?,
            ..Default::default()
        };
        if self.has_fill(node) {
            defaults.color = Some(match self.fill(node)? {
                Some(Fill::Solid(c)) => Some(c),
                _ => None,
            });
            if defaults.color == Some(None) {
                self.warn("unresolved text color: using paragraph/default fallback color".into());
            }
        }
        if let Some(latin) = self.single(node, dml, "latin")? {
            self.attributes(latin, &["typeface"]);
            self.leaf(latin)?;
            let family = latin
                .attribute("typeface")
                .ok_or_else(|| self.invalid("latin missing typeface"))?;
            if family.len() > 256 || family.chars().any(char::is_control) {
                return Err(self.invalid("invalid font family"));
            }
            defaults.family = Some(crate::theme_xml::font(
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
        self.other_children(
            node,
            &[
                (dml, "latin"),
                (dml, "solidFill"),
                (dml, "noFill"),
                (dml, "gradFill"),
                (dml, "blipFill"),
                (dml, "pattFill"),
                (dml, "grpFill"),
            ],
        );
        Ok(defaults)
    }

    pub(super) fn text_style(
        &mut self,
        node: Node<'_, '_>,
        style: TextStyle,
    ) -> Result<TextStyle, PptxError> {
        let mut defaults = self.run_defaults(node)?;
        let unavailable_color = defaults.color == Some(None);
        defaults.inherit(&super::PlaceholderTextDefaults {
            family: Some(style.family),
            font_size: Some(style.font_size),
            color: Some(Some(style.color)),
            bold: Some(style.bold),
            italic: Some(style.italic),
            alignment: None,
        });
        let mut effective = defaults.style();
        if unavailable_color {
            effective.color = style.color;
        }
        Ok(effective)
    }
}
