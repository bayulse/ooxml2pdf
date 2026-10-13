use o2p_core::{Bullet, Color, Point, Rect, Size, VerticalAlignment};
use o2p_fonts::{FontError, FontFace, FontRequest, FontResolver, FontSelection};
use o2p_layout::{Alignment, Paragraph, TextRun, layout_text_box};
use std::{path::PathBuf, sync::Arc};

// Existing OFL scalable outlines with synthetic vertical tables. Horizontal
// tables, name identity, cmap and shaping features are byte-for-byte preserved.
fn face(metrics: [i16; 3]) -> Arc<FontFace> {
    let mut bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/O2PTestSans-Regular.ttf"),
    )
    .unwrap();
    let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
    let offset = |tag: &[u8; 4]| {
        let r = (0..usize::from(count))
            .map(|i| 12 + i * 16)
            .find(|&r| &bytes[r..r + 4] == tag)
            .unwrap();
        u32::from_be_bytes(bytes[r + 8..r + 12].try_into().unwrap()) as usize
    };
    let h = offset(b"hhea");
    let o = offset(b"OS/2");
    // Version 4's USE_TYPO_METRICS and hhea disagree on purpose.
    bytes[o..o + 2].copy_from_slice(&4u16.to_be_bytes());
    bytes[o + 62..o + 64].copy_from_slice(&0x80u16.to_be_bytes());
    for (i, value) in metrics.into_iter().enumerate() {
        bytes[o + 68 + i * 2..o + 70 + i * 2].copy_from_slice(&value.to_be_bytes());
        bytes[h + 4 + i * 2..h + 6 + i * 2].copy_from_slice(&0i16.to_be_bytes());
    }
    Arc::new(FontFace::from_bytes(bytes.into()).unwrap())
}
struct Fonts {
    short: Arc<FontFace>,
    tall: Arc<FontFace>,
}
impl FontResolver for Fonts {
    fn resolve(&self, request: &FontRequest, cluster: &str) -> Result<FontSelection, FontError> {
        Ok(FontSelection {
            font: if request.family == "Tall" || cluster == "Ω" {
                self.tall.clone()
            } else {
                self.short.clone()
            },
            warnings: Vec::new(),
        })
    }
}
fn fonts() -> Fonts {
    Fonts {
        short: face([800, -200, 100]),
        tall: face([900, -400, 150]),
    }
}
fn run(text: &str, family: &str, size: f32) -> TextRun {
    TextRun {
        text: text.into(),
        font: FontRequest {
            family: family.into(),
            ..Default::default()
        },
        font_size: size,
        color: Color::BLACK,
    }
}
fn paragraph(runs: Vec<TextRun>) -> Paragraph {
    Paragraph {
        runs,
        bullet: None,
        format: Default::default(),
        alignment: Alignment::Left,
        empty_font: run("", "Short", 20.0).font,
        empty_font_size: 20.0,
    }
}
fn bounds() -> Rect {
    Rect {
        origin: Point { x: 15.0, y: 25.0 },
        size: Size {
            width: 400.0,
            height: 100.0,
        },
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 0.00002, "{a} != {b}");
}

#[test]
fn top_center_bottom_single_line_distinguish_top_baseline_descent_gap_and_bottom() {
    // Already-inset box: outer y=20, height=112, top=5, bottom=7.
    for (anchor, top) in [
        (VerticalAlignment::Top, 25.0),
        (VerticalAlignment::Center, 64.0),
        (VerticalAlignment::Bottom, 103.0),
    ] {
        let report = layout_text_box(
            &[paragraph(vec![run("Hello", "Short", 20.0)])],
            bounds(),
            anchor,
            &fonts(),
        )
        .unwrap();
        let line = &report.lines[0];
        near(report.height, 22.0);
        near(line.top, top);
        near(f64::from(line.baseline.y), top + 16.0);
        near(line.metrics.ascent, 16.0);
        near(line.metrics.descent, 4.0);
        near(line.metrics.line_gap, 2.0);
        near(line.bottom, top + 22.0);
        assert_eq!(line.runs[0].origin.y, line.baseline.y);
        assert!(!report.overflow);
    }
}
#[test]
fn center_multiline_height_and_paragraph_spacing_use_separate_font_sizes() {
    let mut p = paragraph(vec![run("Hello\nWorld", "Short", 20.0)]);
    p.format.space_before = o2p_core::ParagraphSpacing::TextMultiple(0.5);
    p.format.space_after = o2p_core::ParagraphSpacing::Points(6.0);
    let report = layout_text_box(&[p], bounds(), VerticalAlignment::Center, &fonts()).unwrap();
    near(report.height, 60.0); // 22+22+10+6, not percentages of line height.
    near(report.lines[0].top, 55.0);
    near(report.lines[1].top, 77.0);
    near(f64::from(report.lines[0].baseline.y), 71.0);
    near(f64::from(report.lines[1].baseline.y), 93.0);
    near(report.lines[1].bottom, 99.0);
}
#[test]
fn mixed_fonts_and_mixed_sizes_take_independent_maxima_and_share_baselines() {
    for (short_size, expected) in [(20.0, [18.0, 8.0, 3.0]), (40.0, [32.0, 8.0, 4.0])] {
        let report = layout_text_box(
            &[paragraph(vec![
                run("small ", "Short", short_size),
                run("tall", "Tall", 20.0),
            ])],
            bounds(),
            VerticalAlignment::Top,
            &fonts(),
        )
        .unwrap();
        let line = &report.lines[0];
        near(line.metrics.ascent, expected[0]);
        near(line.metrics.descent, expected[1]);
        near(line.metrics.line_gap, expected[2]);
        near(line.height, expected.iter().sum());
        for run in &line.runs {
            near(f64::from(run.origin.y), 25.0 + expected[0]);
        }
    }
}
#[test]
fn fallback_faces_within_one_semantic_run_participate_in_line_metrics() {
    let report = layout_text_box(
        &[paragraph(vec![run("AΩ", "Short", 20.0)])],
        bounds(),
        VerticalAlignment::Top,
        &fonts(),
    )
    .unwrap();
    assert_eq!(report.lines[0].runs[0].text.runs.len(), 2);
    near(report.height, 29.0);
    near(f64::from(report.lines[0].baseline.y), 43.0);
}

#[test]
fn redistributable_japanese_and_arabic_fallbacks_use_both_extents() {
    let mut book = o2p_fonts::FontBook::new();
    book.register_directory(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
        .unwrap();
    book.set_fallback_families(vec![
        "O2P Test Sans".into(),
        "O2P Test Japanese".into(),
        "O2P Test Arabic".into(),
    ]);
    let p = paragraph(vec![run("A日本مرحبا", "O2P Test Sans", 20.0)]);
    let report = layout_text_box(&[p], bounds(), VerticalAlignment::Top, &book).unwrap();
    let line = &report.lines[0];
    // Japanese hhea ascent=1160; Arabic flagged typo descent=-634, UPEM=1000.
    near(line.metrics.ascent, 23.2);
    near(line.metrics.descent, 12.68);
    near(line.metrics.line_gap, 0.0);
    near(line.height, 35.88);
    assert!(line.runs[0].text.runs.len() >= 3);
    assert!(
        line.runs[0]
            .text
            .runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .all(|g| g.id != 0)
    );
}
#[test]
fn blank_lines_use_end_font_and_size_without_painting_glyphs() {
    let mut p = paragraph(vec![run("Hello\n", "Short", 20.0)]);
    p.empty_font = run("", "Tall", 10.0).font;
    p.empty_font_size = 10.0;
    let report = layout_text_box(&[p], bounds(), VerticalAlignment::Top, &fonts()).unwrap();
    near(report.height, 36.5);
    near(f64::from(report.lines[1].baseline.y), 56.0);
    assert!(report.lines[1].runs.is_empty());
}
#[test]
fn bullet_metrics_expand_first_line_without_changing_text_spacing_or_widths() {
    let mut p = paragraph(vec![run("Hello\nWorld", "Short", 20.0)]);
    p.bullet = Some(Bullet {
        glyph: "•".into(),
        font: run("", "Tall", 30.0).font,
        font_size: 30.0,
        color: Color::BLACK,
    });
    p.format.space_before = o2p_core::ParagraphSpacing::TextMultiple(0.5);
    let report = layout_text_box(&[p], bounds(), VerticalAlignment::Top, &fonts()).unwrap();
    near(report.height, 75.5);
    near(f64::from(report.lines[0].baseline.y), 62.0);
    near(report.lines[1].height, 22.0);
    assert_eq!(
        report.lines[0].runs[0].origin.y,
        report.lines[0].runs[1].origin.y
    );
}
#[test]
fn changing_only_vertical_tables_preserves_wrapping_widths_and_glyph_positions_in_x() {
    let original = fonts();
    let altered = Fonts {
        short: face([1200, -600, 250]),
        tall: face([1200, -600, 250]),
    };
    let mut b = bounds();
    b.size.width = 85.0;
    b.size.height = 1000.0;
    let mut p = paragraph(vec![
        run("Hello wide words ", "Short", 20.0),
        run("with mixed runs", "Tall", 16.0),
    ]);
    p.alignment = Alignment::Center;
    let a = layout_text_box(&[p.clone()], b, VerticalAlignment::Top, &original).unwrap();
    let z = layout_text_box(&[p], b, VerticalAlignment::Top, &altered).unwrap();
    assert_eq!(a.lines.len(), z.lines.len());
    for (a, z) in a.lines.iter().zip(&z.lines) {
        assert_eq!(a.width, z.width);
        assert_eq!(a.baseline.x, z.baseline.x);
        for (a, z) in a.runs.iter().zip(&z.runs) {
            assert_eq!(a.origin.x, z.origin.x);
            assert_eq!(a.text.source, z.text.source);
            assert_eq!(a.text.advance, z.text.advance);
            for (a, z) in a.text.runs.iter().zip(&z.text.runs) {
                assert_eq!(a.origin, z.origin);
                assert_eq!(a.glyphs, z.glyphs);
            }
        }
    }
}
