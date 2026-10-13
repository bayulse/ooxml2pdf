use super::*;

pub const FULL_TEXT: &str = "<a:defRPr sz='3200' b='1' i='1'><a:latin typeface='+mj-lt'/><a:solidFill><a:schemeClr val='tx1'/></a:solidFill></a:defRPr>";
pub fn color_mapping(tx: &str, accent: &str) -> String {
    format!(
        "bg1='lt1' tx1='{tx}' bg2='lt2' tx2='dk2' accent1='{accent}' accent2='accent2' accent3='accent3' accent4='accent4' accent5='accent5' accent6='accent6' hlink='hlink' folHlink='folHlink'"
    )
}
pub fn append_root(parts: &mut Parts, part: &str, root: &str, content: &str) {
    let bytes = &mut parts.iter_mut().find(|(name, _)| name == part).unwrap().1;
    *bytes = String::from_utf8(bytes.clone())
        .unwrap()
        .replace(&format!("</p:{root}>"), &format!("{content}</p:{root}>"))
        .into_bytes();
}
pub fn text_style_deck(kind: &str, styles: &str, local: &str, layout: &str, master: &str) -> Parts {
    let slide = placeholder(
        2,
        kind,
        Some(1),
        "",
        Some(&inherited_paragraph("Inherited 日本語", local)),
    );
    let layout = placeholder(
        2,
        kind,
        Some(1),
        FULL_GEOMETRY,
        Some(&inherited_paragraph("Layout prompt", layout)),
    );
    let master = placeholder(
        2,
        kind,
        None,
        FULL_GEOMETRY,
        Some(&inherited_paragraph("Master prompt", master)),
    );
    let mut parts = attach_theme(
        inheritance_deck("", &slide, "", &layout, "", &master),
        &theme_xml(),
    );
    append_root(
        &mut parts,
        MASTER_PART,
        "sldMaster",
        &format!(
            "<p:clrMap {}/><p:txStyles>{styles}</p:txStyles>",
            color_mapping("accent2", "accent1")
        ),
    );
    parts
}
pub fn master_style(role: &str) -> String {
    format!("<p:{role}><a:lvl1pPr algn='ctr'>{FULL_TEXT}</a:lvl1pPr></p:{role}>")
}
pub fn map_deck() -> Parts {
    let local = format!(
        "{}{}{}",
        text_shape(
            2,
            TEXT_RECT,
            &inherited_paragraph(
                "Mapped",
                "<a:pPr><a:defRPr sz='2000'><a:latin typeface='+mn-lt'/><a:solidFill><a:schemeClr val='tx1'/></a:solidFill></a:defRPr></a:pPr>"
            ),
            "",
            ""
        ),
        rectangle(
            3,
            NONTRIVIAL_RECT,
            "<a:solidFill><a:schemeClr val='accent1'/></a:solidFill>",
            "<a:ln w='25400'><a:solidFill><a:schemeClr val='tx1'/></a:solidFill></a:ln>"
        ),
        styled(
            &rectangle(4, NONTRIVIAL_RECT, "", ""),
            "<a:fillRef idx='1'><a:schemeClr val='accent1'/></a:fillRef><a:lnRef idx='1'><a:schemeClr val='tx1'/></a:lnRef>"
        )
    );
    let mut parts = attach_theme(inheritance_deck("", &local, "", "", "", ""), &style_theme());
    append_root(
        &mut parts,
        MASTER_PART,
        "sldMaster",
        &format!("<p:clrMap {}/>", color_mapping("accent2", "accent3")),
    );
    parts
}
pub fn fidelity2a_fixtures() -> Vec<(&'static str, Parts)> {
    let body = master_style("bodyStyle");
    let inherited = || text_style_deck("body", &body, "", "", "");
    let levels = text_style_deck(
        "body",
        &format!(
            "<p:bodyStyle><a:lvl1pPr>{FULL_TEXT}</a:lvl1pPr><a:lvl3pPr algn='r'><a:defRPr sz='2100'><a:latin typeface='+mn-lt'/><a:solidFill><a:schemeClr val='accent3'/></a:solidFill></a:defRPr></a:lvl3pPr></p:bodyStyle>"
        ),
        "<a:pPr lvl='2'/>",
        "",
        "",
    );
    // Keep the fixture deliberately level-specific; level three does not use
    // level one's properties when those properties are absent.
    let mut local = inherited();
    let bytes = &mut local
        .iter_mut()
        .find(|(n, _)| n == "ppt/slides/slide1.xml")
        .unwrap()
        .1;
    *bytes = String::from_utf8(bytes.clone()).unwrap().replace("<a:r><a:t>", "<a:r><a:rPr sz='1700' b='0' i='0'><a:latin typeface='Local Font'/><a:solidFill><a:srgbClr val='AABBCC'/></a:solidFill></a:rPr><a:t>").into_bytes();
    let mut other = text_style_deck("body", &master_style("otherStyle"), "", "", "");
    replace(
        &mut other,
        "ppt/slides/slide1.xml",
        slide(
            "",
            &text_shape(
                2,
                TEXT_RECT,
                &inherited_paragraph("Inherited 日本語", ""),
                "<a:noFill/>",
                "<a:ln><a:noFill/></a:ln>",
            ),
        ),
    );
    let mut result = vec![
        (
            "master-title-style.pptx",
            text_style_deck("title", &master_style("titleStyle"), "", "", ""),
        ),
        ("master-body-style.pptx", inherited()),
        ("master-other-style.pptx", other),
        (
            "inherited-font-size.pptx",
            text_style_deck(
                "body",
                &body,
                "",
                "<a:pPr><a:defRPr sz='2600'/></a:pPr>",
                "",
            ),
        ),
        (
            "inherited-font-family.pptx",
            text_style_deck(
                "body",
                &body,
                "",
                "<a:pPr><a:defRPr><a:latin typeface='+mn-lt'/></a:defRPr></a:pPr>",
                "",
            ),
        ),
        (
            "inherited-bold-italic.pptx",
            text_style_deck("body", &body, "", "<a:pPr><a:defRPr b='0'/></a:pPr>", ""),
        ),
        (
            "inherited-text-color.pptx",
            text_style_deck(
                "body",
                &body,
                "",
                "<a:pPr><a:defRPr><a:solidFill><a:schemeClr val='accent3'/></a:solidFill></a:defRPr></a:pPr>",
                "",
            ),
        ),
        ("paragraph-level-style.pptx", levels),
        ("local-text-override.pptx", local),
        (
            "layout-placeholder-text-override.pptx",
            text_style_deck(
                "body",
                &body,
                "",
                "<a:pPr algn='r'><a:defRPr sz='2400' b='0'/></a:pPr>",
                "",
            ),
        ),
    ];
    let family = &mut result
        .iter_mut()
        .find(|(name, _)| *name == "inherited-font-family.pptx")
        .unwrap()
        .1;
    let theme = &mut family
        .iter_mut()
        .find(|(part, _)| part == THEME_PART)
        .unwrap()
        .1;
    *theme = String::from_utf8(theme.clone())
        .unwrap()
        .replace(
            "<a:minorFont><a:latin typeface='O2P Test Sans'",
            "<a:minorFont><a:latin typeface='O2P Test Japanese'",
        )
        .into_bytes();
    for name in [
        "master-clrmap.pptx",
        "clrmap-text.pptx",
        "clrmap-fill.pptx",
        "clrmap-line.pptx",
        "clrmap-style-matrix.pptx",
    ] {
        result.push((name, map_deck()));
    }
    for (name, part, root, mapping) in [
        (
            "layout-master-clr-mapping.pptx",
            LAYOUT_PART,
            "sldLayout",
            "<a:masterClrMapping/>".to_owned(),
        ),
        (
            "layout-override-clr-mapping.pptx",
            LAYOUT_PART,
            "sldLayout",
            format!(
                "<a:overrideClrMapping {}/>",
                color_mapping("lt1", "accent4")
            ),
        ),
        (
            "slide-master-clr-mapping.pptx",
            "ppt/slides/slide1.xml",
            "sld",
            "<a:masterClrMapping/>".to_owned(),
        ),
        (
            "slide-override-clr-mapping.pptx",
            "ppt/slides/slide1.xml",
            "sld",
            format!(
                "<a:overrideClrMapping {}/>",
                color_mapping("accent6", "accent5")
            ),
        ),
    ] {
        let mut parts = map_deck();
        if part == "ppt/slides/slide1.xml" {
            append_root(
                &mut parts,
                LAYOUT_PART,
                "sldLayout",
                &format!(
                    "<p:clrMapOvr><a:overrideClrMapping {}/></p:clrMapOvr>",
                    color_mapping("lt1", "accent4")
                ),
            );
        }
        append_root(
            &mut parts,
            part,
            root,
            &format!("<p:clrMapOvr>{mapping}</p:clrMapOvr>"),
        );
        result.push((name, parts));
    }
    result
}
