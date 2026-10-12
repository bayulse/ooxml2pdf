use lopdf::{Document, content::Content};
use o2p_core::{Color, DisplayList, DrawOp, FontResource, GlyphTextOp, Page, Point, Size};
use o2p_fonts::{FontBook, FontRequest};
use o2p_pdf::{PdfError, render, render_with_report};
use std::{path::PathBuf, sync::Arc};

fn book() -> FontBook {
    let mut book = FontBook::new();
    book.register_directory(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
        .unwrap();
    book.set_fallback_families(vec![
        "O2P Test Sans".into(),
        "O2P Test Japanese".into(),
        "O2P Test Arabic".into(),
    ]);
    book
}
fn display(source: &str, book: &FontBook) -> DisplayList {
    let text = o2p_fonts::shape(
        book,
        source,
        &FontRequest {
            family: "O2P Test Sans".into(),
            ..FontRequest::default()
        },
        18.0,
    )
    .unwrap()
    .text;
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 720.0,
                height: 300.0,
            },
            ops: vec![DrawOp::GlyphText(GlyphTextOp {
                origin: Point { x: 20.0, y: 40.0 },
                text,
                color: Color::BLACK,
            })],
        }],
    }
}
fn operation(display: &mut DisplayList) -> &mut GlyphTextOp {
    let DrawOp::GlyphText(op) = &mut display.pages[0].ops[0] else {
        panic!()
    };
    op
}

#[test]
fn true_type_embedding_cid_mapping_searchable_unicode_and_actual_text() {
    let registry = book();
    for source in [
        "ASCII",
        "Résumé",
        "Привет Ελληνικά",
        "日本語",
        "ffi",
        "a\u{301}\u{323}",
        "e\u{301} é",
        "Latin 日本語",
    ] {
        let display = display(source, &registry);
        let bytes = render(&display).unwrap();
        assert_eq!(bytes, render(&display).unwrap());
        let pdf = Document::load_mem(&bytes).unwrap();
        // lopdf does not implement ActualText replacement for continuation
        // glyphs; a conforming independent reader is tested below for these.
        if source != "a\u{301}\u{323}" {
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().trim_end(),
                source,
                "{source}"
            );
        }
        let page = pdf.get_pages()[&1];
        let content = Content::decode(&pdf.get_page_content(page).unwrap()).unwrap();
        assert!(content.operations.iter().any(|o| o.operator == "Tj"));
        assert!(!content.operations.iter().any(|o| o.operator == "Do"));
        let actual = content
            .operations
            .iter()
            .find(|o| o.operator == "BDC")
            .unwrap()
            .operands[1]
            .as_dict()
            .unwrap()
            .get(b"ActualText")
            .unwrap()
            .as_str()
            .unwrap();
        let expected: Vec<_> = [0xfe, 0xff]
            .into_iter()
            .chain(source.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(actual, expected);
        let (resources, _) = pdf.get_page_resources(page).unwrap();
        let fonts = resources.unwrap().get(b"Font").unwrap().as_dict().unwrap();
        for (name, reference) in fonts.iter().filter(|(name, _)| name.starts_with(b"E")) {
            let font = pdf
                .get_dictionary(reference.as_reference().unwrap())
                .unwrap();
            assert_eq!(font.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0");
            assert_eq!(
                font.get(b"Encoding").unwrap().as_name().unwrap(),
                b"Identity-H"
            );
            let cmap = pdf
                .get_object(font.get(b"ToUnicode").unwrap().as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap();
            assert!(
                cmap.content.windows(11).any(|w| w == b"beginbfchar"),
                "{name:?}"
            );
            let descendant = pdf
                .get_dictionary(
                    font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0]
                        .as_reference()
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(
                descendant.get(b"Subtype").unwrap().as_name().unwrap(),
                b"CIDFontType2"
            );
            let gid_map = pdf
                .get_object(
                    descendant
                        .get(b"CIDToGIDMap")
                        .unwrap()
                        .as_reference()
                        .unwrap(),
                )
                .unwrap()
                .as_stream()
                .unwrap();
            assert!(gid_map.content.len() >= 4);
            let descriptor = pdf
                .get_dictionary(
                    descendant
                        .get(b"FontDescriptor")
                        .unwrap()
                        .as_reference()
                        .unwrap(),
                )
                .unwrap();
            let program = pdf
                .get_object(
                    descriptor
                        .get(b"FontFile2")
                        .unwrap()
                        .as_reference()
                        .unwrap(),
                )
                .unwrap()
                .as_stream()
                .unwrap();
            assert!(
                registry
                    .fonts()
                    .iter()
                    .any(|f| program.content == f.resource().bytes.as_ref())
            );
        }
    }
}

#[test]
fn complex_cluster_copy_search_with_poppler_when_available() {
    if std::process::Command::new("pdftotext")
        .arg("-v")
        .output()
        .is_err()
    {
        return;
    }
    let registry = book();
    let directory =
        std::env::temp_dir().join(format!("o2p-text-extraction-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("text.pdf");
    for source in ["ffi a\u{301}\u{323} e\u{301} é", "日本語", "Latin 日本語"] {
        std::fs::write(&path, render(&display(source, &registry)).unwrap()).unwrap();
        let result = std::process::Command::new("pdftotext")
            .args(["-enc", "UTF-8", "-raw"])
            .arg(&path)
            .arg("-")
            .output()
            .unwrap();
        assert!(result.status.success());
        let extracted = String::from_utf8(result.stdout).unwrap();
        let cleaned: String = extracted
            .chars()
            .filter(|c| !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
            .collect();
        assert_eq!(cleaned.trim_end_matches(['\n', '\r', '\u{c}']), source);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rtl_glyphs_keep_logical_actual_text_and_positioned_advances_offsets() {
    let registry = book();
    let display = display("Latin مرحبا 日本語", &registry);
    let DrawOp::GlyphText(op) = &display.pages[0].ops[0] else {
        panic!()
    };
    let pdf = Document::load_mem(&render(&display).unwrap()).unwrap();
    let content = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    let positions: Vec<_> = content
        .operations
        .iter()
        .filter(|o| o.operator == "Tm")
        .collect();
    assert_eq!(
        positions.len(),
        op.text.runs.iter().map(|r| r.glyphs.len()).sum()
    );
    let mut index = 0;
    for run in &op.text.runs {
        let mut x = op.origin.x + run.origin.x;
        let mut y = op.origin.y + run.origin.y;
        for glyph in &run.glyphs {
            let tm = positions[index];
            assert!((tm.operands[4].as_float().unwrap() - x - glyph.offset.x).abs() < 0.001);
            assert!(
                (tm.operands[5].as_float().unwrap() - (300.0 - y - glyph.offset.y)).abs() < 0.001
            );
            x += glyph.advance.x;
            y += glyph.advance.y;
            index += 1;
        }
    }
}

#[test]
fn resources_deduplicate_and_pdf_is_stable_across_new_registries() {
    let first = book();
    let second = book();
    let mut input = display("A日本語مرحبا", &first);
    let duplicate = input.pages[0].ops[0].clone();
    input.pages[0].ops.push(duplicate);
    let pdf = Document::load_mem(&render(&input).unwrap()).unwrap();
    assert_eq!(
        pdf.objects
            .values()
            .filter_map(|o| o.as_dict().ok())
            .filter(|d| d.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Type0"))
            .count(),
        3
    );
    assert_eq!(
        render(&display("A日本語مرحبا", &first)).unwrap(),
        render(&display("A日本語مرحبا", &second)).unwrap()
    );
}

#[test]
fn malformed_font_glyph_source_geometry_and_missing_glyphs_are_reported() {
    let registry = book();
    let mut input = display("A", &registry);
    operation(&mut input).text.runs[0].font = Arc::new(FontResource {
        bytes: Arc::from(b"not a font".as_slice()),
    });
    assert!(matches!(render(&input), Err(PdfError::InvalidFont { .. })));
    let mut input = display("A", &registry);
    operation(&mut input).text.runs[0].glyphs[0].source = 0..100;
    assert!(matches!(render(&input), Err(PdfError::InvalidFont { .. })));
    let mut input = display("Résumé", &registry);
    operation(&mut input).text.runs[0].glyphs[0].source = 1..2;
    assert!(matches!(render(&input), Err(PdfError::InvalidFont { .. })));
    let mut input = display("A", &registry);
    operation(&mut input).text.runs[0].glyphs[0].id = u16::MAX;
    assert!(matches!(render(&input), Err(PdfError::InvalidFont { .. })));
    let mut input = display("A", &registry);
    operation(&mut input).text.runs[0].glyphs[0].offset.x = f32::NAN;
    assert!(matches!(
        render(&input),
        Err(PdfError::InvalidGeometry { .. })
    ));
    let missing = display("😀", &registry);
    assert!(matches!(
        render(&missing),
        Err(PdfError::UnsupportedText { .. })
    ));
    let report = render_with_report(&missing).unwrap();
    assert_eq!(report.warnings[0].missing_glyphs, ['😀']);
    assert_eq!(
        Document::load_mem(&report.bytes)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .trim_end(),
        "😀"
    );
}
