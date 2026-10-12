use super::*;

pub const LAYOUT_PART: &str = "ppt/slideLayouts/layout.xml";
pub const MASTER_PART: &str = "ppt/slideMasters/master.xml";
pub const FULL_GEOMETRY: &str =
    "<a:xfrm><a:off x='914400' y='1270000'/><a:ext cx='5080000' cy='1270000'/></a:xfrm>";
pub const INHERITED_STYLE: &str = "<a:pPr algn='ctr'><a:defRPr sz='2800' b='1' i='1'><a:latin typeface='O2P Test Sans'/><a:solidFill><a:srgbClr val='2060D0'/></a:solidFill></a:defRPr></a:pPr>";

pub fn placeholder(
    id: usize,
    kind: &str,
    idx: Option<u32>,
    properties: &str,
    paragraphs: Option<&str>,
) -> String {
    let idx = idx.map(|n| format!(" idx='{n}'")).unwrap_or_default();
    let body = paragraphs
        .map(|p| format!("<p:txBody><a:bodyPr/><a:lstStyle/>{p}</p:txBody>"))
        .unwrap_or_default();
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id='{id}' name='Placeholder'/><p:cNvSpPr/><p:nvPr><p:ph type='{kind}'{idx}/></p:nvPr></p:nvSpPr><p:spPr>{properties}</p:spPr>{body}</p:sp>"
    )
}
pub fn inherited_paragraph(text: &str, ppr: &str) -> String {
    format!("<a:p>{ppr}<a:r><a:t>{text}</a:t></a:r></a:p>")
}
pub fn inheritance_deck(
    slide_bg: &str,
    slide_shapes: &str,
    layout_bg: &str,
    layout_shapes: &str,
    master_bg: &str,
    master_shapes: &str,
) -> Parts {
    let mut parts = content_deck(slide_bg, slide_shapes);
    let types = parts
        .iter_mut()
        .find(|(n, _)| n == "[Content_Types].xml")
        .unwrap();
    types.1 = String::from_utf8(types.1.clone()).unwrap().replace("</Types>", &format!("<Override PartName='/{LAYOUT_PART}' ContentType='application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml'/><Override PartName='/{MASTER_PART}' ContentType='application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml'/></Types>")).into_bytes();
    for (part, root, bg, shapes) in [
        (LAYOUT_PART, "sldLayout", layout_bg, layout_shapes),
        (MASTER_PART, "sldMaster", master_bg, master_shapes),
    ] {
        let xml = String::from_utf8(slide(bg, shapes))
            .unwrap()
            .replace("p:sld", &format!("p:{root}"));
        parts.push((part.into(), xml.into_bytes()));
    }
    parts.push((
        "ppt/slides/_rels/slide1.xml.rels".into(),
        rels(&relation(
            "rLayout",
            "slideLayout",
            "../slideLayouts/layout.xml",
        )),
    ));
    parts.push((
        "ppt/slideLayouts/_rels/layout.xml.rels".into(),
        rels(&relation(
            "rMaster",
            "slideMaster",
            "../slideMasters/master.xml",
        )),
    ));
    parts
}

pub fn inheritance_fixtures() -> Vec<(&'static str, Parts)> {
    let text = inherited_paragraph("Slide content", "");
    let slide_ph = placeholder(2, "body", Some(1), "", Some(&text));
    let layout = placeholder(2, "body", Some(1), FULL_GEOMETRY, None);
    let master = placeholder(2, "body", None, FULL_GEOMETRY, None);
    let deck = |local: &str, layout: &str, master: &str| {
        inheritance_deck("", local, "", layout, "", master)
    };
    let mut malformed_layout = deck(&slide_ph, &layout, &master);
    replace(
        &mut malformed_layout,
        "ppt/slides/_rels/slide1.xml.rels",
        rels(&relation("rLayout", "slideLayout", "../slides/slide1.xml")),
    );
    let mut malformed_master = deck(&slide_ph, &layout, &master);
    replace(
        &mut malformed_master,
        "ppt/slideLayouts/_rels/layout.xml.rels",
        rels(&relation(
            "rMaster",
            "slideMaster",
            "../slideLayouts/layout.xml",
        )),
    );
    let decorative_master = rectangle(
        5,
        (0, 0, 9_144_000, 254_000),
        &solid("2060D0"),
        "<a:ln><a:noFill/></a:ln>",
    );
    let decorative_layout = ellipse(
        5,
        (7_620_000, 508_000, 762_000, 762_000),
        &solid("F0B020"),
        "<a:ln><a:noFill/></a:ln>",
    );
    let styled_layout = placeholder(
        2,
        "body",
        Some(1),
        FULL_GEOMETRY,
        Some(&inherited_paragraph(
            "Layout sample never rendered",
            INHERITED_STYLE,
        )),
    );
    let local_position = placeholder(
        2,
        "body",
        Some(1),
        "<a:xfrm><a:off x='2540000'/></a:xfrm>",
        Some(&text),
    );
    let local_style = placeholder(
        2,
        "body",
        Some(1),
        &format!("<a:xfrm><a:ext cx='3810000'/></a:xfrm>{}", solid("FFEEDD")),
        Some(&inherited_paragraph(
            "Local override",
            "<a:pPr algn='r'><a:defRPr sz='2000' b='0' i='0'/></a:pPr>",
        )),
    );
    let mut demo = inheritance_deck(
        "",
        &format!(
            "{}{}",
            placeholder(
                2,
                "title",
                Some(0),
                "",
                Some(&inherited_paragraph("Layout and master inheritance", ""))
            ),
            placeholder(
                3,
                "body",
                Some(1),
                &format!("<a:xfrm><a:off y='3810000'/></a:xfrm>{}", solid("FFF0DC")),
                Some(&inherited_paragraph(
                    "Local override: orange text on a pale box. Geometry and font family come from the layout.",
                    "<a:pPr><a:defRPr sz='2200' b='0' i='0'><a:solidFill><a:srgbClr val='B04C00'/></a:solidFill></a:defRPr></a:pPr>"
                ))
            )
        ),
        "",
        &format!(
            "{}{}{}",
            placeholder(
                2,
                "title",
                Some(0),
                "<a:xfrm><a:off x='635000' y='1270000'/><a:ext cx='7874000' cy='1016000'/></a:xfrm>",
                Some(&inherited_paragraph(
                    "Layout title sample",
                    "<a:pPr><a:defRPr sz='3200'><a:latin typeface='+mj-lt'/></a:defRPr></a:pPr>"
                ))
            ),
            placeholder(
                3,
                "body",
                Some(1),
                "<a:xfrm><a:off x='635000' y='2540000'/><a:ext cx='7874000' cy='2032000'/></a:xfrm>",
                None
            ),
            decorative_layout
        ),
        &background("EAF1F8"),
        &format!(
            "{}{}{}",
            placeholder(
                2,
                "title",
                None,
                "",
                Some(&inherited_paragraph(
                    "Master title sample",
                    "<a:pPr><a:defRPr b='1'><a:solidFill><a:schemeClr val='accent1'/></a:solidFill></a:defRPr></a:pPr>"
                ))
            ),
            placeholder(
                3,
                "body",
                None,
                "",
                Some(&inherited_paragraph(
                    "Master body sample",
                    "<a:pPr><a:defRPr sz='2400'><a:latin typeface='+mn-lt'/></a:defRPr></a:pPr>"
                ))
            ),
            decorative_master
        ),
    );
    demo = attach_theme(demo, &theme_xml());
    replace(
        &mut demo,
        "ppt/_rels/presentation.xml.rels",
        rels(&relation("rId1", "slide", "slides/slide1.xml")),
    );
    demo.push((
        "ppt/slideMasters/_rels/master.xml.rels".into(),
        rels(&relation("rTheme", "theme", "../theme/synthetic.xml")),
    ));
    vec![
        (
            "layout-inherited-position.pptx",
            deck(
                &placeholder(
                    2,
                    "body",
                    Some(1),
                    "<a:xfrm><a:ext cx='3810000' cy='1016000'/></a:xfrm>",
                    Some(&text),
                ),
                &layout,
                &master,
            ),
        ),
        (
            "layout-inherited-size.pptx",
            deck(
                &placeholder(
                    2,
                    "body",
                    Some(1),
                    "<a:xfrm><a:off x='254000' y='508000'/></a:xfrm>",
                    Some(&text),
                ),
                &layout,
                &master,
            ),
        ),
        (
            "master-inherited-position.pptx",
            deck(
                &slide_ph,
                &placeholder(2, "body", Some(1), "", None),
                &master,
            ),
        ),
        (
            "inherited-text-style.pptx",
            deck(&slide_ph, &styled_layout, &master),
        ),
        (
            "local-overrides-layout.pptx",
            deck(&local_position, &styled_layout, &master),
        ),
        (
            "local-overrides-master.pptx",
            deck(
                &local_style,
                &placeholder(2, "body", Some(1), "", None),
                &placeholder(
                    2,
                    "body",
                    None,
                    FULL_GEOMETRY,
                    Some(&inherited_paragraph("Master sample", INHERITED_STYLE)),
                ),
            ),
        ),
        (
            "master-background.pptx",
            inheritance_deck("", &slide_ph, "", &layout, &background("EAF1F8"), &master),
        ),
        (
            "master-decorative-shape.pptx",
            deck(&slide_ph, &layout, &format!("{master}{decorative_master}")),
        ),
        (
            "layout-decorative-shape.pptx",
            deck(&slide_ph, &format!("{layout}{decorative_layout}"), &master),
        ),
        (
            "placeholder-no-duplication.pptx",
            deck(
                &slide_ph,
                &styled_layout,
                &placeholder(
                    2,
                    "body",
                    None,
                    FULL_GEOMETRY,
                    Some(&inherited_paragraph(
                        "Master sample never rendered",
                        INHERITED_STYLE,
                    )),
                ),
            ),
        ),
        (
            "multiple-placeholders.pptx",
            deck(
                &format!(
                    "{slide_ph}{}",
                    placeholder(
                        3,
                        "body",
                        Some(2),
                        "",
                        Some(&inherited_paragraph("Second body", ""))
                    )
                ),
                &format!(
                    "{layout}{}",
                    placeholder(
                        3,
                        "body",
                        Some(2),
                        "<a:xfrm><a:off x='2540000' y='3810000'/><a:ext cx='3810000' cy='1270000'/></a:xfrm>",
                        None
                    )
                ),
                &master,
            ),
        ),
        ("malformed-layout-relationship.pptx", malformed_layout),
        ("malformed-master-relationship.pptx", malformed_master),
        ("layout-master-inheritance.pptx", demo),
    ]
}
