//! Original minimal themes; all package writing remains test-only.
use super::*;

pub const THEME_PART: &str = "ppt/theme/synthetic.xml";
pub const THEME_COLORS: [(&str, &str); 12] = [
    ("dk1", "101820"),
    ("lt1", "F7F9FC"),
    ("dk2", "24334A"),
    ("lt2", "E8EDF3"),
    ("accent1", "2060D0"),
    ("accent2", "209060"),
    ("accent3", "E88A20"),
    ("accent4", "C43D56"),
    ("accent5", "8060B0"),
    ("accent6", "209EAE"),
    ("hlink", "0000FF"),
    ("folHlink", "800080"),
];

pub fn theme_xml() -> String {
    let colors: String = THEME_COLORS
        .into_iter()
        .map(|(slot, value)| {
            let color = if matches!(slot, "dk1" | "lt1") {
                format!(
                    "<a:sysClr val='{}' lastClr='{value}'/>",
                    if slot == "dk1" {
                        "windowText"
                    } else {
                        "window"
                    }
                )
            } else {
                format!("<a:srgbClr val='{value}'/>")
            };
            format!("<a:{slot}>{color}</a:{slot}>")
        })
        .collect();
    let fonts = "<a:latin typeface='O2P Test Sans'/><a:ea typeface=''/><a:cs typeface=''/><a:font script='Jpan' typeface='O2P Test Japanese'/>";
    format!(
        "<a:theme xmlns:a='{DML}' name='Synthetic'><a:themeElements><a:clrScheme name='Synthetic'>{colors}</a:clrScheme><a:fontScheme name='Synthetic'><a:majorFont>{fonts}</a:majorFont><a:minorFont>{fonts}</a:minorFont></a:fontScheme><a:fmtScheme name='Unsupported'><a:fillStyleLst/><a:lnStyleLst/><a:effectStyleLst/><a:bgFillStyleLst/></a:fmtScheme></a:themeElements></a:theme>"
    )
}

pub fn attach_theme(mut parts: Parts, theme: &str) -> Parts {
    for (name, bytes) in &mut parts {
        if name == "[Content_Types].xml" {
            *bytes = String::from_utf8(bytes.clone()).unwrap().replace("</Types>", &format!("<Override PartName='/{THEME_PART}' ContentType='application/vnd.openxmlformats-officedocument.theme+xml'/></Types>")).into_bytes();
        } else if name == "ppt/_rels/presentation.xml.rels" {
            *bytes = String::from_utf8(bytes.clone())
                .unwrap()
                .replace(
                    "</Relationships>",
                    &format!(
                        "{}</Relationships>",
                        relation("rTheme", "theme", "theme/synthetic.xml")
                    ),
                )
                .into_bytes();
        }
    }
    parts.push((THEME_PART.into(), theme.as_bytes().to_vec()));
    parts
}

pub fn scheme(slot: &str, transforms: &str) -> String {
    format!("<a:solidFill><a:schemeClr val='{slot}'>{transforms}</a:schemeClr></a:solidFill>")
}

pub fn theme_text(
    id: usize,
    coordinates: (i64, i64, i64, i64),
    token: &str,
    slot: &str,
    label: &str,
) -> String {
    text_shape(id, coordinates, &format!("<a:p><a:r><a:rPr sz='2200'><a:latin typeface='{token}'/>{}</a:rPr><a:t>{label}</a:t></a:r></a:p>", scheme(slot, "")), "<a:noFill/>", "<a:ln><a:noFill/></a:ln>").replace("wrap='none'", "wrap='square'")
}

pub fn theme_fixtures() -> Vec<(&'static str, Parts)> {
    let no_line = "<a:ln><a:noFill/></a:ln>";
    let rect = rectangle(2, NONTRIVIAL_RECT, &scheme("accent1", ""), no_line);
    let theme = theme_xml();
    let themed = |shapes: &str| attach_theme(content_deck("", shapes), &theme);
    let outline = format!("<a:ln w='38100'>{}</a:ln>", scheme("accent2", ""));
    let palette: String = THEME_COLORS
        .into_iter()
        .enumerate()
        .map(|(i, (slot, _))| {
            rectangle(
                i + 2,
                (
                    381_000 + i as i64 % 6 * 1_397_000,
                    762_000 + i as i64 / 6 * 1_397_000,
                    1_143_000,
                    1_143_000,
                ),
                &scheme(slot, ""),
                no_line,
            )
        })
        .collect();
    let background = format!("<p:bg><p:bgPr>{}</p:bgPr></p:bg>", scheme("lt1", ""));
    let demo = format!(
        "{}{}{}{}{}",
        theme_text(
            2,
            (508_000, 254_000, 8_128_000, 635_000),
            "+mj-lt",
            "dk2",
            "Theme colors and fonts — Résumé"
        ),
        palette,
        theme_text(
            20,
            (508_000, 4_191_000, 8_128_000, 762_000),
            "+mj-lt",
            "accent1",
            "Major Latin: O2P Test Sans"
        ),
        theme_text(
            21,
            (508_000, 5_080_000, 8_128_000, 762_000),
            "+mn-lt",
            "accent2",
            "Minor Latin: O2P Test Sans"
        ),
        straight_line(22, (508_000, 6_096_000, 8_128_000, 0), &outline)
    );
    vec![
        ("theme-accent-fill.pptx", themed(&rect)),
        (
            "theme-outline.pptx",
            themed(&format!(
                "{}{}",
                ellipse(2, NONTRIVIAL_RECT, "<a:noFill/>", &outline),
                straight_line(3, TEXT_RECT, &outline)
            )),
        ),
        (
            "theme-text-color.pptx",
            themed(&theme_text(
                2,
                TEXT_RECT,
                "O2P Test Sans",
                "accent4",
                "Theme text color",
            )),
        ),
        ("multiple-theme-colors.pptx", themed(&palette)),
        (
            "theme-major-font.pptx",
            themed(&theme_text(
                2,
                TEXT_RECT,
                "+mj-lt",
                "dk1",
                "Major theme font",
            )),
        ),
        (
            "theme-minor-font.pptx",
            themed(&theme_text(
                2,
                TEXT_RECT,
                "+mn-lt",
                "dk2",
                "Minor theme font",
            )),
        ),
        (
            "theme-color-transform.pptx",
            themed(&rectangle(
                2,
                NONTRIVIAL_RECT,
                &scheme(
                    "accent1",
                    "<a:lumMod val='60000'/><a:lumOff val='40000'/><a:tint val='80000'/><a:shade val='90000'/>",
                ),
                no_line,
            )),
        ),
        (
            "missing-theme.pptx",
            content_deck(
                "",
                &format!(
                    "{rect}{}",
                    theme_text(3, TEXT_RECT, "+mj-lt", "dk1", "Missing theme fallback")
                ),
            ),
        ),
        (
            "malformed-theme.pptx",
            attach_theme(
                content_deck("", &rect),
                &theme.replace("lastClr='101820'", "lastClr='invalid'"),
            ),
        ),
        (
            "theme-colors-and-fonts.pptx",
            attach_theme(content_deck(&background, &demo), &theme),
        ),
    ]
}
