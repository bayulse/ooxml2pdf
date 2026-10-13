//! XML adapter for sparse placeholder properties. Content is never inherited.
use super::*;
use crate::inheritance::{PlaceholderGeometry, Properties};

impl Parser<'_> {
    pub(super) fn hidden_shape(&self, node: Node<'_, '_>) -> Result<bool, PptxError> {
        for name in ["nvSpPr", "nvCxnSpPr"] {
            if let Some(nv) = self.single(node, self.pml, name)? {
                if let Some(properties) = self.single(nv, self.pml, "cNvPr")? {
                    return Ok(self.boolean(properties, "hidden")? == Some(true));
                }
            }
        }
        Ok(false)
    }
    pub(super) fn boolean(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Result<Option<bool>, PptxError> {
        node.attribute(name)
            .map(|value| match value.trim() {
                "1" | "true" => Ok(true),
                "0" | "false" => Ok(false),
                _ => Err(self.invalid(format!("invalid boolean {name}"))),
            })
            .transpose()
    }

    fn sparse_geometry(
        &mut self,
        node: Node<'_, '_>,
    ) -> Result<(PlaceholderGeometry, bool), PptxError> {
        self.attributes(node, &["rot", "flipH", "flipV"]);
        self.element_only(node)?;
        let mut geometry = PlaceholderGeometry {
            flip_h: self.boolean(node, "flipH")?,
            flip_v: self.boolean(node, "flipV")?,
            ..Default::default()
        };
        if node.attribute("rot").is_some() {
            geometry.rotation = Some(crate::drawingml_rotation_to_degrees(self.integer(
                node,
                "rot",
                i64::from(i32::MIN),
                i64::from(i32::MAX),
            )? as i32));
        }
        for (element, names, targets, minimum) in [
            (
                "off",
                ["x", "y"],
                [&mut geometry.x, &mut geometry.y],
                -27_273_042_329_600,
            ),
            (
                "ext",
                ["cx", "cy"],
                [&mut geometry.width, &mut geometry.height],
                0,
            ),
        ] {
            if let Some(child) = self.single(node, self.dml, element)? {
                self.attributes(child, &names);
                self.leaf(child)?;
                for (name, target) in names.into_iter().zip(targets) {
                    if child.attribute(name).is_some() {
                        *target = Some(emu_to_points(self.integer(
                            child,
                            name,
                            minimum,
                            27_273_042_316_900,
                        )?));
                    }
                }
            }
        }
        let blocked =
            node.attributes()
                .any(|a| a.namespace().is_some() || !["rot", "flipH", "flipV"].contains(&a.name()))
                || node.children().filter(|n| n.is_element()).any(|n| {
                    !n.has_tag_name((self.dml, "off")) && !n.has_tag_name((self.dml, "ext"))
                });
        self.other_children(node, &[(self.dml, "off"), (self.dml, "ext")]);
        Ok((geometry, blocked))
    }

    pub(super) fn placeholder(
        &mut self,
        node: Node<'_, '_>,
        ancestor: bool,
    ) -> Result<Option<Placeholder>, PptxError> {
        let Some(nv) = self.single(node, self.pml, "nvSpPr")? else {
            return Ok(None);
        };
        let Some(nvpr) = self.single(nv, self.pml, "nvPr")? else {
            return Ok(None);
        };
        let Some(ph) = self.single(nvpr, self.pml, "ph")? else {
            return Ok(None);
        };
        self.attributes(ph, &["type", "idx", "orient", "sz", "hasCustomPrompt"]);
        self.leaf(ph)?;
        let kind = ph.attribute("type").unwrap_or("obj").to_owned();
        if !matches!(
            kind.as_str(),
            "title" | "ctrTitle" | "body" | "subTitle" | "obj" | "object" | "text"
        ) {
            self.warn(format!("unsupported placeholder type {}: identity preserved; specialized content is not synthesized", xml::expanded(None, &kind)));
        }
        for name in ["orient", "sz", "hasCustomPrompt"] {
            if ph.attribute(name).is_some() {
                self.warn(format!(
                    "placeholder@{name}: specialized placeholder semantics unsupported"
                ));
            }
        }
        let idx = if ph.attribute("idx").is_some() {
            Some(self.integer(ph, "idx", 0, i64::from(u32::MAX))? as u32)
        } else {
            None
        };
        let mut properties = Properties {
            references: self.style_defaults(node)?.unwrap_or_default(),
            ..Default::default()
        };
        self.attributes(node, &[]);
        self.other_children(
            node,
            &[
                (self.pml, "nvSpPr"),
                (self.pml, "spPr"),
                (self.pml, "txBody"),
                (self.pml, "style"),
                (self.dml, "style"),
            ],
        );
        self.attributes(nv, &[]);
        self.other_children(
            nv,
            &[
                (self.pml, "cNvPr"),
                (self.pml, "cNvSpPr"),
                (self.pml, "nvPr"),
            ],
        );
        self.attributes(nvpr, &[]);
        self.other_children(nvpr, &[(self.pml, "ph")]);
        if let Some(sp) = self.single(node, self.pml, "spPr")? {
            self.attributes(sp, &[]);
            self.other_children(
                sp,
                &[
                    (self.dml, "xfrm"),
                    (self.dml, "prstGeom"),
                    (self.dml, "solidFill"),
                    (self.dml, "noFill"),
                    (self.dml, "gradFill"),
                    (self.dml, "blipFill"),
                    (self.dml, "pattFill"),
                    (self.dml, "grpFill"),
                    (self.dml, "ln"),
                ],
            );
            if let Some(xfrm) = self.single(sp, self.dml, "xfrm")? {
                (properties.geometry, properties.geometry_blocked) = self.sparse_geometry(xfrm)?;
            }
            if let Some(geom) = self.single(sp, self.dml, "prstGeom")? {
                self.attributes(geom, &["prst"]);
                self.element_only(geom)?;
                let preset = geom
                    .attribute("prst")
                    .ok_or_else(|| self.invalid("prstGeom missing prst"))?;
                if !matches!(preset, "rect" | "ellipse" | "line") {
                    self.warn(format!(
                        "unsupported placeholder preset {}",
                        xml::expanded(None, preset)
                    ));
                }
                properties.preset = Some(preset.into());
                for child in geom.children().filter(|n| n.is_element()) {
                    if child.has_tag_name((self.dml, "avLst"))
                        && !child.children().any(|n| n.is_element())
                    {
                        self.attributes(child, &[]);
                        self.leaf(child)?;
                    } else {
                        self.unsupported(child);
                        properties.geometry_blocked = true;
                    }
                }
            }
            if sp.children().any(|n| {
                n.is_element()
                    && n.tag_name().namespace() == Some(self.dml)
                    && matches!(n.tag_name().name(), "custGeom" | "scene3d" | "sp3d")
            }) {
                properties.geometry_blocked = true;
            }
            if self.has_fill(sp) {
                properties.fill = Some(self.fill(sp)?);
            }
            if let Some(line) = self.single(sp, self.dml, "ln")? {
                self.line_properties(line, &mut properties)?;
            }
        }
        if let Some(body) = self.single(node, self.pml, "txBody")? {
            (properties.text_levels, properties.insets) = self.body_properties(body, ancestor)?;
        }
        Ok(Some(Placeholder {
            kind,
            idx,
            resolved_text: properties.effective().text_levels.levels[0].clone(),
            properties,
        }))
    }

    pub(super) fn line_properties(
        &mut self,
        line: Node<'_, '_>,
        properties: &mut Properties,
    ) -> Result<(), PptxError> {
        self.attributes(line, &["w", "cap", "cmpd", "algn"]);
        self.element_only(line)?;
        if line.attribute("w").is_some() {
            properties.line_width = Some(emu_to_points(self.integer(line, "w", 0, 20_116_800)?));
        }
        if self.has_fill(line) {
            properties.line_color = Some(match self.fill(line)? {
                Some(Fill::Solid(c)) => Some(c),
                _ => None,
            });
        }
        for name in ["cap", "cmpd", "algn"] {
            if line.attribute(name).is_some_and(|v| {
                !matches!(
                    (name, v),
                    ("cap", "flat") | ("cmpd", "sng") | ("algn", "ctr")
                )
            }) {
                self.warn(format!(
                    "placeholder outline {name}: unsupported line semantics"
                ));
                properties.line_color = Some(None);
            }
        }
        for child in line.children().filter(|n| {
            n.is_element()
                && !(n.tag_name().namespace() == Some(self.dml)
                    && matches!(
                        n.tag_name().name(),
                        "solidFill" | "noFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                    ))
        }) {
            if child.has_tag_name((self.dml, "prstDash")) && child.attribute("val") == Some("solid")
            {
                self.attributes(child, &["val"]);
                self.leaf(child)?;
            } else {
                self.unsupported(child);
                properties.line_color = Some(None);
            }
        }
        Ok(())
    }

    pub(super) fn has_fill(&self, node: Node<'_, '_>) -> bool {
        node.children().any(|n| {
            n.is_element()
                && n.tag_name().namespace() == Some(self.dml)
                && matches!(
                    n.tag_name().name(),
                    "solidFill" | "noFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
                )
        })
    }

    pub(super) fn paragraph_defaults(
        &mut self,
        ppr: Node<'_, '_>,
    ) -> Result<PlaceholderTextDefaults, PptxError> {
        let mut defaults = PlaceholderTextDefaults::default();
        self.attributes(ppr, &["algn", "lvl"]);
        self.element_only(ppr)?;
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
        if let Some(style) = self.single(ppr, self.dml, "defRPr")? {
            let alignment = defaults.alignment;
            defaults = self.run_defaults(style)?;
            defaults.alignment = alignment;
        }
        self.other_children(ppr, &[(self.dml, "defRPr")]);
        Ok(defaults)
    }

    pub(super) fn list_style(
        &mut self,
        list: Node<'_, '_>,
    ) -> Result<crate::TextListStyle, PptxError> {
        self.attributes(list, &[]);
        self.element_only(list)?;
        let base = if let Some(node) = self.single(list, self.dml, "defPPr")? {
            self.paragraph_defaults(node)?
        } else {
            PlaceholderTextDefaults::default()
        };
        let mut result = crate::TextListStyle::default();
        for (index, level) in result.levels.iter_mut().enumerate() {
            let name = format!("lvl{}pPr", index + 1);
            if let Some(node) = self.single(list, self.dml, &name)? {
                *level = self.paragraph_defaults(node)?;
            }
            level.inherit(&base);
        }
        for node in list.children().filter(|n| n.is_element()) {
            if node.tag_name().namespace() != Some(self.dml)
                || !matches!(
                    node.tag_name().name(),
                    "defPPr"
                        | "lvl1pPr"
                        | "lvl2pPr"
                        | "lvl3pPr"
                        | "lvl4pPr"
                        | "lvl5pPr"
                        | "lvl6pPr"
                        | "lvl7pPr"
                        | "lvl8pPr"
                        | "lvl9pPr"
                )
            {
                self.unsupported(node);
            }
        }
        Ok(result)
    }

    pub(super) fn master_text_styles(
        &mut self,
        root: Node<'_, '_>,
    ) -> Result<crate::MasterTextStyles, PptxError> {
        let mut styles = crate::MasterTextStyles::default();
        if let Some(node) = self.single(root, self.pml, "txStyles")? {
            self.attributes(node, &[]);
            self.element_only(node)?;
            for (name, target) in [
                ("titleStyle", &mut styles.title),
                ("bodyStyle", &mut styles.body),
                ("otherStyle", &mut styles.other),
            ] {
                if let Some(list) = self.single(node, self.pml, name)? {
                    *target = self.list_style(list)?;
                }
            }
            self.other_children(
                node,
                &[
                    (self.pml, "titleStyle"),
                    (self.pml, "bodyStyle"),
                    (self.pml, "otherStyle"),
                ],
            );
        }
        Ok(styles)
    }

    pub(super) fn body_properties(
        &mut self,
        body: Node<'_, '_>,
        ancestor: bool,
    ) -> Result<(crate::TextListStyle, crate::text_defaults::InsetDefaults), PptxError> {
        let mut levels = if let Some(list) = self.single(body, self.dml, "lstStyle")? {
            self.list_style(list)?
        } else {
            crate::TextListStyle::default()
        };
        let mut insets = crate::text_defaults::InsetDefaults::default();
        if let Some(node) = self.single(body, self.dml, "bodyPr")? {
            for (name, target) in ["lIns", "rIns", "tIns", "bIns"]
                .into_iter()
                .zip(&mut insets.0)
            {
                if node.attribute(name).is_some() {
                    *target = Some(emu_to_points(self.integer(
                        node,
                        name,
                        0,
                        i64::from(i32::MAX),
                    )?));
                }
            }
            if ancestor {
                self.attributes(node, &["lIns", "rIns", "tIns", "bIns", "anchor", "wrap"]);
                for (name, supported) in [("anchor", "t"), ("wrap", "square")] {
                    if node.attribute(name).is_some_and(|v| v != supported) {
                        self.warn(format!(
                            "unsupported inherited bodyPr {name}: using supported textbox defaults"
                        ));
                    }
                }
                self.element_only(node)?;
                for child in node.children().filter(|n| n.is_element()) {
                    if child.has_tag_name((self.dml, "noAutofit")) {
                        self.attributes(child, &[]);
                        self.leaf(child)?;
                    } else {
                        self.unsupported(child);
                    }
                }
            }
        }
        if ancestor {
            self.attributes(body, &[]);
            self.element_only(body)?;
            self.other_children(
                body,
                &[
                    (self.dml, "bodyPr"),
                    (self.dml, "lstStyle"),
                    (self.dml, "p"),
                ],
            );
            let mut seen = [false; 9];
            let dml = self.dml;
            for paragraph in body.children().filter(|n| n.has_tag_name((dml, "p"))) {
                if let Some(ppr) = self.single(paragraph, self.dml, "pPr")? {
                    let level = self.paragraph_level(Some(ppr))?;
                    let mut local = self.paragraph_defaults(ppr)?;
                    if !seen[level] {
                        local.inherit(&levels.levels[level]);
                        levels.levels[level] = local;
                        seen[level] = true;
                    } else {
                        self.warn(format!("ancestor placeholder has multiple paragraphs at level {level}: first paragraph defaults inherited"));
                    }
                }
                if paragraph
                    .children()
                    .filter(|n| n.has_tag_name((self.dml, "r")))
                    .any(|n| n.children().any(|r| r.has_tag_name((self.dml, "rPr"))))
                {
                    self.warn("placeholder run formatting is content-specific and is not inherited; use paragraph defRPr".into());
                }
            }
        }
        Ok((levels, insets))
    }

    pub(super) fn placeholder_shapes(
        &mut self,
        node: Node<'_, '_>,
        ph: &Placeholder,
    ) -> Result<Vec<Shape>, PptxError> {
        let mut effective = ph.properties.effective();
        effective
            .text_levels
            .inherit(self.text_styles.for_kind(&ph.kind));
        let props = &effective;
        if props.geometry_blocked {
            self.warn("unsupported inherited placeholder geometry: element omitted".into());
            return Ok(vec![]);
        }
        let Some((rect, transform)) = props.geometry.resolved() else {
            self.warn("incomplete placeholder geometry: element omitted".into());
            return Ok(vec![]);
        };
        let stroke = props.line_color.flatten().and_then(|color| {
            let width = props.line_width.unwrap_or(0.75);
            (width > 0.0).then_some(Stroke { color, width })
        });
        let mut shapes = vec![];
        match props.preset.as_deref().unwrap_or("text") {
            "rect" => shapes.push(Shape::Rectangle {
                rect,
                fill: props.fill.flatten(),
                stroke,
            }),
            "ellipse" => shapes.push(Shape::Ellipse {
                bounds: rect,
                fill: props.fill.flatten(),
                stroke,
            }),
            "line" => shapes.push(Shape::Line {
                start: rect.origin,
                end: Point {
                    x: rect.origin.x + rect.size.width,
                    y: rect.origin.y + rect.size.height,
                },
                stroke,
            }),
            "text" => {
                // Rectangular text-box paint does not require preset geometry.
                if matches!(props.fill.flatten(), Some(Fill::Solid(_))) || stroke.is_some() {
                    shapes.push(Shape::Rectangle {
                        rect,
                        fill: props.fill.flatten(),
                        stroke,
                    });
                }
            }
            _ => {
                self.warn("unsupported inherited placeholder preset: element omitted".into());
                return Ok(vec![]);
            }
        }
        if let Some(body) = self.single(node, self.pml, "txBody")? {
            if let Some(text) = self.text_box(body, rect, Some(props))? {
                shapes.push(Shape::TextBox(text));
            }
        }
        Ok(transformed(transform, shapes))
    }
}
