//! Original, deterministic style-matrix fixtures; production never writes ZIPs.
use super::*;

pub const STYLE_MATRIX: &str = "<a:fmtScheme name='Synthetic styles'><a:fillStyleLst><a:solidFill><a:schemeClr val='phClr'/></a:solidFill><a:solidFill><a:schemeClr val='accent2'><a:shade val='50000'/></a:schemeClr></a:solidFill><a:noFill/></a:fillStyleLst><a:lnStyleLst><a:ln w='25400'><a:solidFill><a:schemeClr val='phClr'/></a:solidFill><a:prstDash val='solid'/></a:ln><a:ln w='50800'><a:solidFill><a:schemeClr val='accent5'/></a:solidFill></a:ln><a:ln w='0'><a:noFill/></a:ln></a:lnStyleLst><a:effectStyleLst/><a:bgFillStyleLst><a:solidFill><a:schemeClr val='lt2'><a:tint val='50000'/></a:schemeClr></a:solidFill><a:solidFill><a:schemeClr val='phClr'/></a:solidFill><a:noFill/></a:bgFillStyleLst></a:fmtScheme>";

pub fn style_theme() -> String {
    let theme = theme_xml();
    let start = theme.find("<a:fmtScheme").unwrap();
    let end = theme.find("</a:fmtScheme>").unwrap() + "</a:fmtScheme>".len();
    format!("{}{}{}", &theme[..start], STYLE_MATRIX, &theme[end..])
}
pub fn style_ref(name: &str, index: &str, slot: &str) -> String {
    format!("<a:{name} idx='{index}'><a:schemeClr val='{slot}'/></a:{name}>")
}
pub fn styled(shape: &str, references: &str) -> String {
    shape.replace("wrap='none'", "wrap='square'").replace(
        "</p:sp>",
        &format!("<p:style>{references}</p:style></p:sp>"),
    )
}
pub fn style_deck(background: &str, shapes: &str) -> Parts {
    attach_theme(
        inheritance_deck(background, shapes, "", "", "", ""),
        &style_theme(),
    )
}
pub fn style_background(index: &str, slot: &str) -> String {
    format!("<p:bg><p:bgRef idx='{index}'><a:schemeClr val='{slot}'/></p:bgRef></p:bg>")
}

pub fn style_fixtures() -> Vec<(&'static str, Parts)> {
    let fill = style_ref("fillRef", "1", "accent1");
    let line = style_ref("lnRef", "1", "accent4");
    let both = format!("{fill}{line}");
    let base = rectangle(2, NONTRIVIAL_RECT, "", "");
    let inherited = |at_layout: bool| {
        let local = placeholder(
            2,
            "body",
            Some(1),
            "",
            Some(&inherited_paragraph("Inherited style — Résumé", "")),
        );
        let parent = styled(
            &placeholder(
                2,
                "body",
                if at_layout { Some(1) } else { None },
                FULL_GEOMETRY,
                None,
            ),
            &format!("{}{}", both, style_ref("fontRef", "minor", "dk2")),
        );
        let bridge = placeholder(2, "body", Some(1), "", None);
        let decoration = styled(
            &ellipse(3, (7_620_000, 508_000, 762_000, 762_000), "", ""),
            &both,
        );
        let (layout, master) = if at_layout {
            (
                format!("{parent}{decoration}"),
                placeholder(2, "body", None, "", None),
            )
        } else {
            (bridge, format!("{parent}{decoration}"))
        };
        attach_theme(
            inheritance_deck("", &local, "", &layout, "", &master),
            &style_theme(),
        )
    };
    let font = style_ref("fontRef", "major", "dk2");
    let text = text_shape(
        2,
        TEXT_RECT,
        &inherited_paragraph("Theme font reference — Résumé", ""),
        "<a:noFill/>",
        "<a:ln><a:noFill/></a:ln>",
    );
    let title = styled(
        &text_shape(
            2,
            (508_000, 254_000, 8_128_000, 889_000),
            &inherited_paragraph(
                "DrawingML style matrix — Résumé",
                "<a:pPr><a:defRPr sz='3000' b='1' i='1'/></a:pPr>",
            ),
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        ),
        &font,
    );
    let cards: String = [
        ("Theme fill", "accent1", "accent1", "", ""),
        ("Theme line", "accent2", "accent4", "<a:noFill/>", ""),
        (
            "Local override",
            "accent1",
            "accent4",
            "<a:solidFill><a:srgbClr val='E88A20'/></a:solidFill>",
            "<a:ln w='38100'><a:solidFill><a:srgbClr val='24334A'/></a:solidFill></a:ln>",
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(i, (label, fill_slot, line_slot, local_fill, local_line))| {
            let refs = format!(
                "{}{}{}",
                style_ref("fillRef", "1", fill_slot),
                style_ref("lnRef", "1", line_slot),
                style_ref("fontRef", "minor", "dk1")
            );
            let paragraphs = format!(
                "{}{}",
                inherited_paragraph(label, "<a:pPr><a:defRPr sz='2400' b='1' i='1'/></a:pPr>"),
                inherited_paragraph(
                    "Theme styles supply paint. Local formatting has priority.",
                    "<a:pPr><a:defRPr sz='1800' b='0'/></a:pPr>"
                )
            );
            styled(
                &text_shape(
                    i + 3,
                    (
                        508_000 + i as i64 * 2_794_000,
                        1_778_000,
                        2_540_000,
                        3_302_000,
                    ),
                    &paragraphs,
                    local_fill,
                    local_line,
                )
                .replace("wrap='none'", "wrap='square'"),
                &refs,
            )
        },
    )
    .collect();
    let footer = styled(
        &text_shape(
            6,
            (508_000, 5_461_000, 8_128_000, 889_000),
            &inherited_paragraph(
                "Background: bgRef 1001 uses the first bgFillStyleLst entry. Text remains selectable Unicode.",
                "<a:pPr><a:defRPr sz='1800'/></a:pPr>",
            ),
            "<a:noFill/>",
            "<a:ln><a:noFill/></a:ln>",
        ),
        &style_ref("fontRef", "minor", "dk2"),
    );
    let mut gradient = style_deck("", &styled(&base, &fill));
    replace(&mut gradient, THEME_PART, style_theme().replace("<a:solidFill><a:schemeClr val='phClr'/></a:solidFill>", "<a:gradFill><a:gsLst><a:gs pos='0'><a:schemeClr val='phClr'/></a:gs><a:gs pos='100000'><a:schemeClr val='accent2'/></a:gs></a:gsLst><a:lin ang='0' scaled='1'/></a:gradFill>").into_bytes());
    vec![
        ("style-fill-ref.pptx", style_deck("", &styled(&base, &fill))),
        (
            "style-line-ref.pptx",
            style_deck(
                "",
                &styled(&rectangle(2, NONTRIVIAL_RECT, "<a:noFill/>", ""), &line),
            ),
        ),
        (
            "style-fill-and-line-ref.pptx",
            style_deck("", &styled(&base, &both)),
        ),
        (
            "style-local-fill-overrides-ref.pptx",
            style_deck(
                "",
                &styled(&rectangle(2, NONTRIVIAL_RECT, &solid("E88A20"), ""), &both),
            ),
        ),
        (
            "style-local-line-overrides-ref.pptx",
            style_deck(
                "",
                &styled(
                    &rectangle(2, NONTRIVIAL_RECT, "", &outline("24334A", 63500)),
                    &both,
                ),
            ),
        ),
        ("master-style-ref.pptx", inherited(false)),
        ("layout-style-ref.pptx", inherited(true)),
        (
            "background-style-ref.pptx",
            style_deck(&style_background("1001", "accent1"), &styled(&base, &both)),
        ),
        ("theme-font-ref.pptx", style_deck("", &styled(&text, &font))),
        ("unsupported-gradient-style-ref.pptx", gradient),
        (
            "malformed-style-index.pptx",
            style_deck(
                "",
                &styled(&base, &style_ref("fillRef", "invalid", "accent1")),
            ),
        ),
        (
            "style-matrix-demo.pptx",
            style_deck(
                &style_background("1001", "lt2"),
                &format!("{title}{cards}{footer}"),
            ),
        ),
    ]
}
