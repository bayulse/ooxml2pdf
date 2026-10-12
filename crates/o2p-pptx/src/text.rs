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
        inherited: Option<&super::PlaceholderTextDefaults>,
    ) -> Result<Option<TextBox>, PptxError> {
        let dml = self.dml;
        self.attributes(body, &[]);
        self.element_only(body)?;
        self.other_children(body, &[(dml, "bodyPr"), (dml, "lstStyle"), (dml, "p")]);
        let mut insets = TextInsets::default();
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
        if let Some(list) = self.single(body, dml, "lstStyle")? {
            self.attributes(list, &[]);
            self.element_only(list)?;
            if inherited.is_none() {
                self.other_children(list, &[]);
            }
        }
        if inherited.is_none() {
            self.warn("text inheritance/font resolution: fallback is 18pt regular black sans-serif, left/top aligned with 7.2pt horizontal and 3.6pt vertical insets; direct properties override fallback".into());
        }
        let mut paragraphs = vec![];
        for paragraph in body.children().filter(|n| n.has_tag_name((dml, "p"))) {
            paragraphs.push(self.paragraph(paragraph, inherited)?);
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
        inherited: Option<&super::PlaceholderTextDefaults>,
    ) -> Result<Paragraph, PptxError> {
        let dml = self.dml;
        self.attributes(paragraph, &[]);
        self.element_only(paragraph)?;
        let properties = self.single(paragraph, dml, "pPr")?;
        let end = self.single(paragraph, dml, "endParaRPr")?;
        let mut default = inherited.map_or_else(TextStyle::default, |d| d.style());
        let mut alignment = inherited
            .and_then(|d| d.alignment)
            .unwrap_or(TextAlignment::Left);
        if let Some(properties) = properties {
            self.attributes(properties, &["algn", "lvl"]);
            if properties.attribute("lvl").is_some() && self.integer(properties, "lvl", 0, 8)? != 0
            {
                self.warn(
                    "paragraph level styles are unsupported: using generic text defaults".into(),
                );
                default = TextStyle::default();
                alignment = TextAlignment::Left;
            }
            alignment = match properties.attribute("algn") {
                None => alignment,
                Some("l") => TextAlignment::Left,
                Some("ctr") => TextAlignment::Center,
                Some("r") => TextAlignment::Right,
                Some(_) => {
                    self.warn("unsupported paragraph alignment: using left alignment".into());
                    TextAlignment::Left
                }
            };
            if let Some(style) = self.single(properties, dml, "defRPr")? {
                default = self.text_style(style, default.clone())?;
            }
            self.other_children(properties, &[(dml, "defRPr")]);
        }
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
                let style = if let Some(style) = self.single(child, dml, "rPr")? {
                    self.text_style(style, default.clone())?
                } else {
                    default.clone()
                };
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
                let style = if let Some(style) = self.single(child, dml, "rPr")? {
                    self.text_style(style, default.clone())?
                } else {
                    default.clone()
                };
                runs.push(TextRun {
                    text: "\n".into(),
                    style,
                });
            } else {
                self.unsupported(child);
            }
        }
        Ok(Paragraph {
            runs,
            alignment,
            end_style,
        })
    }

    pub(super) fn text_style(
        &mut self,
        node: Node<'_, '_>,
        mut style: TextStyle,
    ) -> Result<TextStyle, PptxError> {
        let dml = self.dml;
        self.attributes(
            node,
            &["sz", "b", "i", "lang", "altLang", "dirty", "smtClean"],
        );
        self.element_only(node)?;
        if node.attribute("sz").is_some() {
            style.font_size = self.integer(node, "sz", 100, 400_000)? as f32 / 100.0;
        }
        for (name, target) in [("b", &mut style.bold), ("i", &mut style.italic)] {
            if let Some(value) = node.attribute(name) {
                *target = match value.trim() {
                    "1" | "true" => true,
                    "0" | "false" => false,
                    _ => return Err(self.invalid(format!("invalid text boolean {name}"))),
                };
            }
        }
        if node.children().any(|child| {
            child.is_element()
                && child.tag_name().namespace() == Some(dml)
                && matches!(
                    child.tag_name().name(),
                    "solidFill" | "noFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                )
        }) {
            if let Some(Fill::Solid(color)) = self.fill(node)? {
                style.color = color;
            } else {
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
            let mut context = crate::theme_xml::Context {
                part: self.reference.part_name(),
                slide_id: Some(self.reference.id()),
                warnings: self.warnings,
            };
            style.family = crate::theme_xml::font(family, self.theme, &mut context);
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
        Ok(style)
    }
}
