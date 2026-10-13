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
        bullet: None,
        format: Default::default(),
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

fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}

#[test]
fn bullet_hanging_indent_and_wrapped_continuations_use_effective_geometry() {
    let book = fonts();
    let mut p = paragraph(vec![run("one two three four five six seven")]);
    p.format.margin_left = 30.0;
    p.format.margin_right = 10.0;
    p.format.first_line_indent = -20.0;
    p.bullet = Some(o2p_core::Bullet {
        glyph: "•".into(),
        font: run("").font,
        font_size: 20.0,
        color: Color::BLACK,
    });
    let report = layout(&[p], bounds(140.0, 500.0), &book).unwrap();
    assert!(report.lines.len() > 2);
    near(report.lines[0].runs[0].origin.x, 21.0);
    for (index, line) in report.lines.iter().enumerate() {
        near(line.baseline.x, 41.0);
        assert!(line.width <= 100.0);
        for r in line.runs.iter().skip(usize::from(index == 0)) {
            near(r.origin.x, 41.0);
        }
    }
    assert_eq!(
        report
            .lines
            .iter()
            .flat_map(|l| &l.runs)
            .filter(|r| r.text.source == "•")
            .count(),
        1
    );
}

#[test]
fn nonbullet_first_line_indent_changes_only_the_first_line_width_and_origin() {
    let book = fonts();
    for indent in [-15.0, 15.0] {
        let mut p = paragraph(vec![run("one two three four five six seven eight nine")]);
        p.format.margin_left = 30.0;
        p.format.first_line_indent = indent;
        let report = layout(&[p], bounds(130.0, 500.0), &book).unwrap();
        assert!(report.lines.len() > 1);
        near(report.lines[0].baseline.x, 41.0 + indent);
        for line in report.lines.iter().skip(1) {
            near(line.baseline.x, 41.0);
        }
    }
}

#[test]
fn bullets_follow_first_line_alignment_without_changing_available_text_width() {
    for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
        let mut p = paragraph(vec![run("Hello")]);
        p.alignment = alignment;
        p.format.margin_left = 30.0;
        p.format.first_line_indent = -20.0;
        let plain = layout(&[p.clone()], bounds(200.0, 100.0), &fonts()).unwrap();
        p.bullet = Some(o2p_core::Bullet {
            glyph: "–".into(),
            font: run("").font,
            font_size: 20.0,
            color: Color::BLACK,
        });
        let bullet = layout(&[p], bounds(200.0, 100.0), &fonts()).unwrap();
        near(
            bullet.lines[0].runs[0].origin.x,
            bullet.lines[0].baseline.x - 20.0,
        );
        assert_eq!(bullet.lines[0].width, plain.lines[0].width);
    }
}

#[test]
fn paragraph_spacing_adds_boxes_without_blank_lines_or_changed_wrapping() {
    let book = fonts();
    let plain = paragraph(vec![run("one two three four five six")]);
    let before = layout(&[plain.clone(), plain.clone()], bounds(120.0, 500.0), &book).unwrap();
    let mut spaced = plain;
    spaced.format.space_before = o2p_core::ParagraphSpacing::Points(10.0);
    spaced.format.space_after = o2p_core::ParagraphSpacing::Points(7.0);
    let after = layout(&[spaced.clone(), spaced], bounds(120.0, 500.0), &book).unwrap();
    assert_eq!(line_sources(&before), line_sources(&after));
    near(after.lines[0].baseline.y - before.lines[0].baseline.y, 10.0);
    assert_eq!(after.height - before.height, 34.0);
    assert!(
        after
            .lines
            .iter()
            .zip(&before.lines)
            .all(|(a, b)| a.height == b.height && a.width == b.width)
    );
}

#[test]
fn percentage_spacing_uses_largest_font_on_first_and_last_lines() {
    let mut big = run("big");
    big.font_size = 30.0;
    let mut p = paragraph(vec![run("small "), big, run("\nlast")]);
    p.format.space_before = o2p_core::ParagraphSpacing::TextMultiple(0.5);
    p.format.space_after = o2p_core::ParagraphSpacing::TextMultiple(0.25);
    let report = layout(&[p], bounds(500.0, 500.0), &fonts()).unwrap();
    assert_eq!(report.lines.len(), 2);
    near(report.lines[0].baseline.y, 52.0);
    assert_eq!(report.height, 80.0);
}

#[test]
fn vertical_anchors_shift_entire_measured_block_and_preserve_overflow() {
    let mut p = paragraph(vec![run("first\nsecond")]);
    p.format.space_before = o2p_core::ParagraphSpacing::Points(10.0);
    p.format.space_after = o2p_core::ParagraphSpacing::Points(6.0);
    for (anchor, offset) in [
        (o2p_core::VerticalAlignment::Top, 0.0),
        (o2p_core::VerticalAlignment::Center, 68.0),
        (o2p_core::VerticalAlignment::Bottom, 136.0),
    ] {
        let report =
            o2p_layout::layout_text_box(&[p.clone()], bounds(200.0, 200.0), anchor, &fonts())
                .unwrap();
        assert_eq!(report.height, 64.0);
        near(report.lines[0].baseline.y, 37.0 + offset);
        near(report.lines[1].baseline.y, 61.0 + offset);
        assert!(!report.overflow);
    }
    let report = o2p_layout::layout_text_box(
        &[p],
        bounds(200.0, 20.0),
        o2p_core::VerticalAlignment::Bottom,
        &fonts(),
    )
    .unwrap();
    assert!(report.overflow);
    near(report.lines[0].baseline.y, -7.0);
}

#[test]
fn empty_bullet_paragraphs_keep_line_height_without_a_glyph() {
    let mut p = paragraph(vec![]);
    p.bullet = Some(o2p_core::Bullet {
        glyph: "•".into(),
        font: run("").font,
        font_size: 20.0,
        color: Color::BLACK,
    });
    let report = layout(&[p], bounds(100.0, 100.0), &fonts()).unwrap();
    assert_eq!(report.height, 24.0);
    assert!(report.lines[0].runs.is_empty());
}

#[test]
fn malformed_geometry_spacing_bullets_and_extreme_arithmetic_are_bounded() {
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let mut p = paragraph(vec![run("x")]);
        p.format.margin_left = value;
        assert_eq!(
            layout(&[p], bounds(100.0, 100.0), &fonts()),
            Err(FontError::InvalidRequest)
        );
        let mut p = paragraph(vec![run("x")]);
        p.format.space_before = o2p_core::ParagraphSpacing::Points(value);
        assert_eq!(
            layout(&[p], bounds(100.0, 100.0), &fonts()),
            Err(FontError::InvalidRequest)
        );
    }
    let mut p = paragraph(vec![run("x")]);
    p.bullet = Some(o2p_core::Bullet {
        glyph: "x".repeat(o2p_layout::MAX_PARAGRAPH_BYTES),
        font: run("").font,
        font_size: 20.0,
        color: Color::BLACK,
    });
    assert!(matches!(
        layout(&[p], bounds(100.0, 100.0), &fonts()),
        Err(FontError::LimitExceeded(_))
    ));
    let mut p = paragraph(vec![run("x")]);
    p.format.space_before = o2p_core::ParagraphSpacing::TextMultiple(f32::MAX);
    assert_eq!(
        layout(&[p], bounds(100.0, 100.0), &fonts()),
        Err(FontError::InvalidRequest)
    );
}

#[test]
fn fixed_font_compatibility_path_keeps_generic_spacing_and_anchor_semantics() {
    let mut p = paragraph(vec![run("first\nsecond")]);
    p.format.margin_left = 30.0;
    p.format.first_line_indent = -20.0;
    p.format.space_before = o2p_core::ParagraphSpacing::Points(10.0);
    p.format.space_after = o2p_core::ParagraphSpacing::Points(6.0);
    p.bullet = Some(o2p_core::Bullet {
        glyph: "•".into(),
        font: run("").font,
        font_size: 20.0,
        color: Color::BLACK,
    });
    let (ops, overflow) = o2p_layout::layout_unwrapped(
        &[p],
        bounds(200.0, 200.0),
        o2p_core::VerticalAlignment::Center,
    );
    assert!(!overflow);
    assert_eq!(ops.len(), 3);
    near(ops[0].origin.x, 21.0);
    near(ops[1].origin.x, 41.0);
    near(ops[2].origin.x, 41.0);
    near(ops[0].origin.y, 105.0);
    near(ops[2].origin.y, 129.0);
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
