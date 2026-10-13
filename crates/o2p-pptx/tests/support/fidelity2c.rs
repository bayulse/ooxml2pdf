use super::*;

pub const TYPOGRAPHY_RUN: &str = "<a:defRPr sz='2000'><a:latin typeface='O2P Test Sans'/><a:solidFill><a:srgbClr val='000000'/></a:solidFill></a:defRPr>";
pub const CHAR_BULLET: &str = "<a:buChar char='•'/>";

pub fn typography_deck(attributes: &str, children: &str, body: &str, text: &str) -> Parts {
    let paragraph = format!(
        "<a:p><a:pPr {attributes}>{children}{TYPOGRAPHY_RUN}</a:pPr><a:r><a:t>{text}</a:t></a:r></a:p>"
    );
    let shape = text_shape(2, TEXT_RECT, &paragraph, "", "").replace(
        "<a:bodyPr wrap='none'><a:noAutofit/></a:bodyPr>",
        &format!("<a:bodyPr {body}><a:noAutofit/></a:bodyPr>"),
    );
    content_deck("", &shape)
}

pub fn typography_inherited(
    kind: &str,
    master_style: &str,
    local: &str,
    layout: &str,
    master: &str,
) -> Parts {
    let role = if kind == "title" {
        "titleStyle"
    } else {
        "bodyStyle"
    };
    text_style_deck(
        kind,
        &format!("<p:{role}><a:lvl1pPr {master_style}>{TYPOGRAPHY_RUN}</a:lvl1pPr></p:{role}>"),
        local,
        layout,
        master,
    )
}

pub fn bullet_inherited(local: &str, layout: &str, master: &str) -> Parts {
    text_style_deck(
        "body",
        &format!(
            "<p:bodyStyle><a:lvl1pPr marL='342900' indent='-228600' algn='l'>{CHAR_BULLET}<a:buFont typeface='O2P Test Sans'/><a:spcBef><a:spcPts val='500'/></a:spcBef>{TYPOGRAPHY_RUN}</a:lvl1pPr></p:bodyStyle>"
        ),
        local,
        layout,
        master,
    )
}

pub fn typography_fixture(name: &str) -> Parts {
    fidelity2c_fixtures()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}

pub fn fidelity2c_fixtures() -> Vec<(&'static str, Parts)> {
    let basic = || {
        typography_deck(
            "marL='342900' indent='-228600'",
            CHAR_BULLET,
            "",
            "Bullet paragraph",
        )
    };
    let mut before = typography_deck(
        "",
        "<a:spcBef><a:spcPts val='1000'/></a:spcBef>",
        "spcFirstLastPara='1'",
        "Before paragraph",
    );
    let mut after = typography_deck(
        "",
        "<a:spcAft><a:spcPts val='800'/></a:spcAft>",
        "spcFirstLastPara='1'",
        "After paragraph",
    );
    for deck in [&mut before, &mut after] {
        let bytes = &mut deck
            .iter_mut()
            .find(|(n, _)| n == "ppt/slides/slide1.xml")
            .unwrap()
            .1;
        *bytes = String::from_utf8(bytes.clone()).unwrap().replace("</p:txBody>", &format!("<a:p><a:pPr>{TYPOGRAPHY_RUN}</a:pPr><a:r><a:t>Second paragraph</a:t></a:r></a:p></p:txBody>")).into_bytes();
    }
    let mut centered = typography_inherited("title", "algn='ctr'", "", "", "");
    let bytes = &mut centered
        .iter_mut()
        .find(|(n, _)| n == MASTER_PART)
        .unwrap()
        .1;
    *bytes = String::from_utf8(bytes.clone())
        .unwrap()
        .replace("<a:bodyPr/>", "<a:bodyPr anchor='ctr'/>")
        .into_bytes();
    let mut rotated = typography_deck(
        "algn='ctr'",
        "",
        "anchor='ctr' lIns='127000' rIns='254000' tIns='63500' bIns='127000'",
        "Rotated centered text",
    );
    let bytes = &mut rotated
        .iter_mut()
        .find(|(n, _)| n == "ppt/slides/slide1.xml")
        .unwrap()
        .1;
    *bytes = String::from_utf8(bytes.clone())
        .unwrap()
        .replace("<a:xfrm>", "<a:xfrm rot='5400000'>")
        .into_bytes();
    vec![
        ("bullet-char.pptx", basic()),
        (
            "bullet-custom-char.pptx",
            typography_deck("", "<a:buChar char='»'/>", "", "Custom Unicode bullet"),
        ),
        (
            "bullet-custom-font.pptx",
            typography_deck(
                "",
                "<a:buChar char='日'/><a:buFont typeface='O2P Test Japanese'/>",
                "",
                "Custom font",
            ),
        ),
        ("inherited-bullet.pptx", bullet_inherited("", "", "")),
        (
            "local-no-bullet.pptx",
            bullet_inherited("<a:pPr><a:buNone/></a:pPr>", "", ""),
        ),
        (
            "bullet-indent.pptx",
            typography_deck(
                "marL='457200' indent='127000'",
                CHAR_BULLET,
                "",
                "Positive indent",
            ),
        ),
        ("hanging-bullet.pptx", basic()),
        (
            "wrapped-bullet.pptx",
            typography_deck(
                "marL='342900' indent='-228600'",
                CHAR_BULLET,
                "",
                "A long paragraph that wraps through several lines to verify that continuation lines align with the paragraph text rather than the bullet glyph. A second sentence continues the same paragraph.",
            ),
        ),
        ("paragraph-space-before.pptx", before),
        ("paragraph-space-after.pptx", after),
        (
            "inherited-paragraph-spacing.pptx",
            bullet_inherited("", "", ""),
        ),
        ("centered-title-placeholder.pptx", centered),
        (
            "textbox-anchor-center.pptx",
            typography_deck("", "", "anchor='ctr'", "Centered text"),
        ),
        (
            "textbox-anchor-bottom.pptx",
            typography_deck("", "", "anchor='b'", "Bottom text"),
        ),
        ("textbox-anchor-rotated.pptx", rotated),
    ]
}
