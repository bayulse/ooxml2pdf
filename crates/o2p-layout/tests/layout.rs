use o2p_core::{Color, Point, Rect, Size};
use o2p_fonts::{FontBook, FontError, FontFace, FontRequest, shape};
use o2p_layout::{Alignment, LayoutReport, Paragraph, TextRun, layout};
use std::{collections::BTreeSet, path::PathBuf};

fn fonts() -> FontBook {
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
fn run(text: &str) -> TextRun {
    TextRun {
        text: text.into(),
        font: FontRequest {
            family: "O2P Test Sans".into(),
            ..Default::default()
        },
        font_size: 20.0,
        color: Color::BLACK,
    }
}
fn paragraph(runs: Vec<TextRun>) -> Paragraph {
    Paragraph {
        runs,
        alignment: Alignment::Left,
        empty_font_size: 20.0,
    }
}
fn bounds(width: f32, height: f32) -> Rect {
    Rect {
        origin: Point { x: 11.0, y: 7.0 },
        size: Size { width, height },
    }
}
fn measured(book: &FontBook, run: &TextRun) -> f32 {
    shape(book, &run.text, &run.font, run.font_size)
        .unwrap()
        .text
        .advance
        .x
}
fn source(report: &LayoutReport) -> String {
    report
        .lines
        .iter()
        .flat_map(|l| &l.runs)
        .map(|r| r.text.source.as_str())
        .collect()
}
fn line_sources(report: &LayoutReport) -> Vec<String> {
    report
        .lines
        .iter()
        .map(|l| l.runs.iter().map(|r| r.text.source.as_str()).collect())
        .collect()
}

#[test]
fn widths_are_shaped_glyph_advances_and_exact_fit_does_not_wrap() {
    let book = fonts();
    for text in ["ffi", "Résumé", "a\u{301}\u{323}", "日本語", "مرحبا"] {
        let run = run(text);
        let shaped = shape(&book, text, &run.font, run.font_size).unwrap().text;
        let glyph_sum: f64 = shaped
            .runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .map(|g| f64::from(g.advance.x))
            .sum();
        assert!((glyph_sum - f64::from(shaped.advance.x)).abs() < 0.00001);
        let report = layout(
            &[paragraph(vec![run])],
            bounds(shaped.advance.x, 100.0),
            &book,
        )
        .unwrap();
        assert_eq!(report.lines.len(), 1, "{text}");
        assert_eq!(report.lines[0].width, f64::from(shaped.advance.x));
        assert!(!report.overflow);
    }
    assert!(measured(&book, &run("WWW")) > measured(&book, &run("iii")) * 2.0);
    let ligature = shape(&book, "ffi", &run("").font, 20.0).unwrap();
    assert!(
        ligature
            .text
            .runs
            .iter()
            .map(|r| r.glyphs.len())
            .sum::<usize>()
            < 3
    );
}

#[test]
fn whitespace_wraps_prefer_whole_words_and_keeps_source() {
    let book = fonts();
    let width = measured(&book, &run("Hello "));
    let report = layout(
        &[paragraph(vec![run("Hello world")])],
        bounds(width, 200.0),
        &book,
    )
    .unwrap();
    assert_eq!(line_sources(&report), ["Hello ", "world"]);
    assert_eq!(source(&report), "Hello world");
    assert!(report.lines.iter().all(|l| l.width <= f64::from(width)));
    assert!(!report.overflow);
}

#[test]
fn one_word_overflow_splits_clusters_and_oversized_cluster_makes_progress() {
    let book = fonts();
    let text = "Supercalifragilistic";
    let width = measured(&book, &run("Super"));
    let report = layout(&[paragraph(vec![run(text)])], bounds(width, 1000.0), &book).unwrap();
    assert!(report.lines.len() > 1);
    assert_eq!(source(&report), text);
    assert!(!report.overflow);
    for width in [0.0, 0.01] {
        let p = paragraph(vec![run("WffiW")]);
        let report = layout(std::slice::from_ref(&p), bounds(width, 1000.0), &book).unwrap();
        assert_eq!(line_sources(&report), ["W", "ffi", "W"]);
        assert!(report.overflow);
        assert_eq!(report, layout(&[p], bounds(width, 1000.0), &book).unwrap());
    }
}

#[test]
fn explicit_breaks_blank_lines_empty_paragraphs_and_empty_boxes_stack() {
    let book = fonts();
    let mut empty = paragraph(vec![]);
    empty.empty_font_size = 10.0;
    let p = paragraph(vec![run("A\n\nB\n")]);
    let report = layout(
        &[p, empty, paragraph(vec![run("C")])],
        bounds(100.0, 1000.0),
        &book,
    )
    .unwrap();
    assert_eq!(line_sources(&report), ["A", "", "B", "", "", "C"]);
    assert_eq!(
        report
            .lines
            .iter()
            .map(|l| l.baseline.y)
            .collect::<Vec<_>>(),
        [27.0, 51.0, 75.0, 99.0, 113.0, 135.0]
    );
    let empty = layout(&[], bounds(0.0, 0.0), &book).unwrap();
    assert!(empty.lines.is_empty() && empty.warnings.is_empty() && !empty.overflow);
    assert!(
        layout(&[paragraph(vec![run("")])], bounds(100.0, 100.0), &book)
            .unwrap()
            .lines[0]
            .runs
            .is_empty()
    );
}

#[test]
fn mixed_styles_survive_wraps_and_alignment_uses_each_final_line() {
    let book = fonts();
    let mut styled = run("bold italic red words across lines ");
    styled.font.weight = 700;
    styled.font.italic = true;
    styled.font_size = 24.0;
    styled.color = Color::rgb(200, 20, 40);
    for (alignment, factor) in [
        (Alignment::Left, 0.0),
        (Alignment::Center, 0.5),
        (Alignment::Right, 1.0),
    ] {
        let mut p = paragraph(vec![run("normal "), styled.clone(), run("normal ending")]);
        p.alignment = alignment;
        let report = layout(&[p], bounds(150.0, 1000.0), &book).unwrap();
        assert!(report.lines.len() >= 3);
        assert_eq!(
            source(&report),
            "normal bold italic red words across lines normal ending"
        );
        let mut styled_lines = 0;
        for line in &report.lines {
            let expected = 11.0 + (150.0 - line.width) * factor;
            assert!((f64::from(line.baseline.x) - expected).abs() < 0.00002);
            let mut x = expected;
            for r in &line.runs {
                assert!((f64::from(r.origin.x) - x).abs() < 0.00002);
                assert_eq!(r.origin.y, line.baseline.y);
                x += f64::from(r.text.advance.x);
                if r.color == styled.color {
                    styled_lines += 1;
                    assert_eq!(r.text.font_size, 24.0);
                    for font in &r.text.runs {
                        let face = FontFace::from_resource(font.font.clone()).unwrap();
                        assert_eq!(face.metadata().weight, 700);
                        assert!(face.metadata().italic);
                    }
                } else {
                    assert_eq!(r.color, Color::BLACK);
                    assert_eq!(r.text.font_size, 20.0);
                }
            }
        }
        assert!(styled_lines >= 2);
        assert!(!report.overflow);
    }
}

#[test]
fn unicode_fallback_cuts_only_at_shaping_and_grapheme_boundaries() {
    let book = fonts();
    for text in [
        "Résumé a\u{301}\u{323} ffi Résumé",
        "日本語日本語",
        "مرحبا مرحبا",
    ] {
        let input = run(text);
        let shaped = shape(&book, text, &input.font, input.font_size)
            .unwrap()
            .text;
        let ranges: Vec<_> = shaped
            .runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .map(|g| g.source.clone())
            .collect();
        let report = layout(&[paragraph(vec![input])], bounds(0.0, 1000.0), &book).unwrap();
        assert_eq!(source(&report), text);
        let mut offset = 0;
        for line in &report.lines {
            let logical: String = line.runs.iter().map(|r| r.text.source.as_str()).collect();
            offset += logical.len();
            assert!(text.is_char_boundary(offset));
            assert!(
                ranges.iter().all(|r| !(r.start < offset && offset < r.end)),
                "cut at {offset} in {text}"
            );
            for r in &line.runs {
                for glyph in r.text.runs.iter().flat_map(|r| &r.glyphs) {
                    assert!(r.text.source.get(glyph.source.clone()).is_some());
                }
            }
        }
    }
    // Even a grapheme whose combining mark starts a differently styled run
    // stays on one line. Cross-style shaping itself remains out of scope.
    let mut combining = run("\u{301}\u{323}");
    combining.color = Color::rgb(0, 0, 200);
    let report = layout(
        &[paragraph(vec![run("a"), combining, run("b")])],
        bounds(0.0, 1000.0),
        &book,
    )
    .unwrap();
    assert_eq!(line_sources(&report), ["a\u{301}\u{323}", "b"]);
}

#[test]
fn rtl_line_edges_are_reshaped_and_overflow_is_deterministic_without_clipping() {
    let book = fonts();
    let p = paragraph(vec![run("مرحبا مرحبا مرحبا")]);
    let report = layout(std::slice::from_ref(&p), bounds(65.0, 1.0), &book).unwrap();
    assert!(report.overflow);
    assert!(report.lines.len() > 1);
    assert_eq!(source(&report), p.runs[0].text);
    assert!(report.lines.last().unwrap().baseline.y > 8.0);
    for line in &report.lines {
        for r in &line.runs {
            assert_eq!(
                r.text,
                shape(&book, &r.text.source, &p.runs[0].font, 20.0)
                    .unwrap()
                    .text
            );
        }
    }
    assert_eq!(report, layout(&[p], bounds(65.0, 1.0), &book).unwrap());
}

#[test]
fn whitespace_across_run_boundaries_has_no_extra_breaks() {
    let book = fonts();
    let width = measured(&book, &run("Hello "));
    let report = layout(
        &[paragraph(vec![run("Hel"), run("lo "), run("world")])],
        bounds(width, 100.0),
        &book,
    )
    .unwrap();
    assert_eq!(line_sources(&report), ["Hello ", "world"]);
    let p = paragraph(vec![run("ab"), run("cd")]);
    let report = layout(&[p], bounds(100.0, 100.0), &book).unwrap();
    assert_eq!(line_sources(&report), ["abcd"]);
    assert_eq!(report.lines[0].runs.len(), 2);
}

#[test]
fn invalid_geometry_and_workload_limits_are_owned_errors() {
    let book = fonts();
    assert_eq!(
        layout(&[], bounds(f32::NAN, 10.0), &book),
        Err(FontError::InvalidRequest)
    );
    assert_eq!(
        layout(&[], bounds(-1.0, 10.0), &book),
        Err(FontError::InvalidRequest)
    );
    let mut p = paragraph(vec![run("A")]);
    p.runs[0].font_size = f32::INFINITY;
    assert_eq!(
        layout(&[p], bounds(10.0, 10.0), &book),
        Err(FontError::InvalidRequest)
    );
    let p = paragraph(vec![run(&"a".repeat(o2p_layout::MAX_PARAGRAPH_BYTES + 1))]);
    assert!(matches!(
        layout(&[p], bounds(10.0, 10.0), &book),
        Err(FontError::LimitExceeded(_))
    ));
    // Many identical glyphs do not change cluster safety or termination.
    let report = layout(
        &[paragraph(vec![run(&"W".repeat(500))])],
        bounds(0.0, 1000.0),
        &book,
    )
    .unwrap();
    assert_eq!(report.lines.len(), 500);
    let unique: BTreeSet<_> = report.lines.iter().map(|l| l.baseline.y as i32).collect();
    assert_eq!(unique.len(), 500);
}
