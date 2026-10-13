use super::*;

pub const SHADOW_LIST: &str = "<a:effectLst><a:outerShdw blurRad='40000' dist='25400' dir='5400000' rotWithShape='0'><a:srgbClr val='102030'><a:alpha val='35000'/></a:srgbClr></a:outerShdw></a:effectLst>";
pub fn effect_shape(shape: &str, effect: &str) -> String {
    shape.replace("</p:spPr>", &format!("{effect}</p:spPr>"))
}
pub fn rounded(shape: &str, adjustment: &str) -> String {
    shape
        .replace("prst='rect'", "prst='roundRect'")
        .replace("<a:avLst/>", &format!("<a:avLst>{adjustment}</a:avLst>"))
}
pub fn shadow_style_deck(shape: &str) -> Parts {
    let theme = style_theme().replace(
        "<a:effectStyleLst/>",
        &format!(
            "<a:effectStyleLst><a:effectStyle>{}</a:effectStyle></a:effectStyleLst>",
            SHADOW_LIST
                .replace("<a:srgbClr val='102030'>", "<a:schemeClr val='phClr'>")
                .replace("</a:srgbClr>", "</a:schemeClr>")
        ),
    );
    attach_theme(inheritance_deck("", shape, "", "", "", ""), &theme)
}
pub fn fidelity2b_fixtures() -> Vec<(&'static str, Parts)> {
    let rect = rectangle(
        2,
        NONTRIVIAL_RECT,
        &solid("2060D0"),
        &outline("C43D56", 25400),
    );
    let round = rounded(&rect, "");
    let text = rounded(
        &text_shape(
            2,
            TEXT_RECT,
            &format!("<a:p>{}</a:p>", text_run("Rounded geometry", "sz='1800'")),
            &solid("2060D0"),
            &outline("C43D56", 25400),
        ),
        "",
    )
    .replace("wrap='none'", "wrap='square'");
    let ref_shape = styled(&round, &style_ref("effectRef", "1", "accent2"));
    let local = SHADOW_LIST.replace("102030", "FF0000");
    vec![
        ("rounded-rectangle.pptx", content_deck("", &round)),
        ("rounded-rectangle-with-text.pptx", content_deck("", &text)),
        (
            "rounded-rectangle-custom-radius.pptx",
            content_deck("", &rounded(&rect, "<a:gd name='adj' fmla='val 12000'/>")),
        ),
        (
            "rotated-rounded-rectangle.pptx",
            content_deck(
                "",
                &round.replace("<a:xfrm>", "<a:xfrm rot='2700000' flipH='1'>"),
            ),
        ),
        (
            "rectangle-outer-shadow.pptx",
            content_deck("", &effect_shape(&rect, SHADOW_LIST)),
        ),
        (
            "rounded-rectangle-shadow.pptx",
            content_deck("", &effect_shape(&round, SHADOW_LIST)),
        ),
        (
            "ellipse-shadow.pptx",
            content_deck(
                "",
                &effect_shape(
                    &ellipse(
                        2,
                        NONTRIVIAL_RECT,
                        &solid("209060"),
                        &outline("C43D56", 25400),
                    ),
                    SHADOW_LIST,
                ),
            ),
        ),
        (
            "image-shadow.pptx",
            image_deck(
                false,
                &effect_shape(&picture(2, NONTRIVIAL_RECT), SHADOW_LIST),
            ),
        ),
        ("theme-shadow-style.pptx", shadow_style_deck(&ref_shape)),
        (
            "local-shadow-overrides-theme.pptx",
            shadow_style_deck(&effect_shape(&ref_shape, &local)),
        ),
        (
            "overlapping-shadow-zorder.pptx",
            content_deck(
                "",
                &format!(
                    "{}{}",
                    effect_shape(&round, SHADOW_LIST),
                    rectangle(
                        3,
                        NONTRIVIAL_RECT,
                        &solid("FFFF00"),
                        "<a:ln><a:noFill/></a:ln>"
                    )
                ),
            ),
        ),
    ]
}
