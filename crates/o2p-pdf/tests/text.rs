use lopdf::{Document, content::Content};
use o2p_core::{Color, DisplayList, DrawOp, Font, Page, Point, Size, TextOp};
use o2p_pdf::{PdfError, render, render_with_report};

fn display(text: &str, font: Font) -> DisplayList {
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 400.0,
                height: 300.0,
            },
            ops: vec![DrawOp::Text(TextOp {
                origin: Point { x: 20.0, y: 40.0 },
                text: text.into(),
                font,
                font_size: 18.0,
                color: Color::rgb(18, 171, 205),
            })],
        }],
    }
}

#[test]
fn unicode_is_encoded_as_winansi_and_roundtrips_through_an_independent_reader() {
    let input = "Hello Indonesia Résumé € — “PDF” Æ œ Š Ÿ";
    let pdf = Document::load_mem(&render(&display(input, Font::SansSerif)).unwrap()).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap().trim_end(), input);
    let page = pdf.get_pages()[&1];
    let content = Content::decode(&pdf.get_page_content(page).unwrap()).unwrap();
    let bytes = content
        .operations
        .iter()
        .find(|o| o.operator == "Tj")
        .unwrap()
        .operands[0]
        .as_str()
        .unwrap();
    assert!(bytes.contains(&0xe9));
    assert!(bytes.contains(&0x80));
    assert!(!bytes.windows(2).any(|s| s == [0xc3, 0xa9])); // never raw UTF-8 in a Type1 font string
    let (resources, _) = pdf.get_page_resources(page).unwrap();
    let fonts = resources.unwrap().get(b"Font").unwrap().as_dict().unwrap();
    let font = pdf
        .get_dictionary(fonts.get(b"F1").unwrap().as_reference().unwrap())
        .unwrap();
    assert_eq!(
        font.get(b"Encoding").unwrap().as_name().unwrap(),
        b"WinAnsiEncoding"
    );
}

#[test]
fn all_basic_faces_are_real_pdf_standard_font_resources() {
    for (face, name, resource) in [
        (Font::SansSerif, "Helvetica", "F1"),
        (Font::SansSerifBold, "Helvetica-Bold", "F2"),
        (Font::SansSerifItalic, "Helvetica-Oblique", "F3"),
        (Font::SansSerifBoldItalic, "Helvetica-BoldOblique", "F4"),
    ] {
        let pdf = Document::load_mem(&render(&display("Styled", face)).unwrap()).unwrap();
        let page = pdf.get_pages()[&1];
        let content = Content::decode(&pdf.get_page_content(page).unwrap()).unwrap();
        let tf = content
            .operations
            .iter()
            .find(|o| o.operator == "Tf")
            .unwrap();
        assert_eq!(tf.operands[0].as_name().unwrap(), resource.as_bytes());
        assert_eq!(tf.operands[1].as_float().unwrap(), 18.0);
        let (resources, _) = pdf.get_page_resources(page).unwrap();
        let fonts = resources.unwrap().get(b"Font").unwrap().as_dict().unwrap();
        let font = pdf
            .get_dictionary(
                fonts
                    .get(resource.as_bytes())
                    .unwrap()
                    .as_reference()
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            font.get(b"BaseFont").unwrap().as_name().unwrap(),
            name.as_bytes()
        );
        let rg = content
            .operations
            .iter()
            .find(|o| o.operator == "rg")
            .unwrap();
        for (actual, channel) in rg.operands.iter().zip([18, 171, 205]) {
            assert!((actual.as_float().unwrap() - channel as f32 / 255.0).abs() < 1e-6);
        }
        let tm = content
            .operations
            .iter()
            .find(|o| o.operator == "Tm")
            .unwrap();
        assert_eq!(tm.operands[4].as_float().unwrap(), 20.0);
        assert_eq!(tm.operands[5].as_float().unwrap(), 260.0);
    }
}

#[test]
fn reporting_renderer_warns_on_missing_glyphs_and_strict_api_keeps_its_error_policy() {
    let input = display("Résumé 日本語 日本語 😀", Font::SansSerifBoldItalic);
    let report = render_with_report(&input).unwrap();
    assert_eq!(report, render_with_report(&input).unwrap());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].missing_glyphs, ['日', '本', '語', '😀']);
    assert_eq!((report.warnings[0].page, report.warnings[0].op), (0, 0));
    let pdf = Document::load_mem(&report.bytes).unwrap();
    assert_eq!(
        pdf.extract_text(&[1]).unwrap().trim_end(),
        "Résumé ??? ??? ?"
    );
    assert_eq!(
        render(&input),
        Err(PdfError::UnsupportedText {
            page: 0,
            op: 0,
            character: '日'
        })
    );
}

#[test]
fn fallback_warnings_follow_page_and_operation_order() {
    let mut input = display("日", Font::SansSerif);
    input.pages[0].ops.push(DrawOp::Text(TextOp {
        origin: Point { x: 0.0, y: 80.0 },
        text: "😀".into(),
        font: Font::SansSerifItalic,
        font_size: 10.0,
        color: Color::BLACK,
    }));
    input.pages.push(input.pages[0].clone());
    let report = render_with_report(&input).unwrap();
    assert_eq!(
        report
            .warnings
            .iter()
            .map(|w| (w.page, w.op))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (1, 0), (1, 1)]
    );
}

#[test]
fn controls_and_invalid_geometry_are_errors_even_in_reporting_mode() {
    for character in ['\0', '\t', '\n', '\u{7f}', '\u{85}'] {
        assert_eq!(
            render_with_report(&display(&character.to_string(), Font::SansSerif)).unwrap_err(),
            PdfError::UnsupportedText {
                page: 0,
                op: 0,
                character
            }
        );
    }
    let mut input = display("Unicode 日本語", Font::SansSerif);
    let DrawOp::Text(t) = &mut input.pages[0].ops[0] else {
        unreachable!()
    };
    t.font_size = f32::NAN;
    assert!(matches!(
        render_with_report(&input),
        Err(PdfError::InvalidGeometry {
            field: "font size",
            ..
        })
    ));
}
