use o2p_fonts::{FontError, FontFace, FontVerticalMetrics, VerticalMetricSource};
use std::{path::PathBuf, sync::Arc};

fn fixture() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/O2PTestSans-Regular.ttf"),
    )
    .unwrap()
}
fn offset(bytes: &[u8], tag: &[u8; 4]) -> usize {
    let n = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
    let record = (0..usize::from(n))
        .map(|i| 12 + i * 16)
        .find(|&i| &bytes[i..i + 4] == tag)
        .unwrap();
    u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize
}
fn put(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn face(
    version: u16,
    selection: u16,
    hhea: [i16; 3],
    typo: [i16; 3],
    win: [u16; 2],
) -> Result<FontFace, FontError> {
    let mut bytes = fixture();
    let h = offset(&bytes, b"hhea");
    let o = offset(&bytes, b"OS/2");
    put(&mut bytes, o, version);
    put(&mut bytes, o + 62, selection);
    for (i, value) in hhea.into_iter().enumerate() {
        put(&mut bytes, h + 4 + i * 2, value as u16);
    }
    for (i, value) in typo.into_iter().enumerate() {
        put(&mut bytes, o + 68 + i * 2, value as u16);
    }
    for (i, value) in win.into_iter().enumerate() {
        put(&mut bytes, o + 74 + i * 2, value);
    }
    FontFace::from_bytes(Arc::from(bytes))
}
#[test]
fn normalization_preserves_signed_design_units_and_positive_point_distances() {
    let raw = FontVerticalMetrics {
        units_per_em: 2048,
        ascender: 1600,
        descender: -400,
        line_gap: 200,
        source: VerticalMetricSource::Typographic,
    };
    let m = raw.at_size(32.0).unwrap();
    assert_eq!(m.ascent, 25.0);
    assert_eq!(m.descent, 6.25);
    assert_eq!(m.line_gap, 3.125);
    assert_eq!(m.height(), 34.375);
    assert_eq!(raw.at_size(16.0).unwrap().height(), m.height() / 2.0);
}
#[test]
fn metric_selection_respects_version_and_use_typo_bit_as_a_complete_triplet() {
    for (version, selection, source, expected) in [
        (4, 0x80, VerticalMetricSource::Typographic, [750, -250, 100]),
        (4, 0, VerticalMetricSource::Hhea, [900, -300, 50]),
        (3, 0x80, VerticalMetricSource::Hhea, [900, -300, 50]),
    ] {
        let f = face(
            version,
            selection,
            [900, -300, 50],
            [750, -250, 100],
            [1000, 400],
        )
        .unwrap();
        let m = f.metadata().vertical_metrics;
        assert_eq!(m.source, source);
        assert_eq!([m.ascender, m.descender, m.line_gap], expected);
    }
}
#[test]
fn missing_hhea_metrics_fall_back_to_whole_typo_then_unsigned_windows_triplets() {
    let f = face(4, 0, [0, 0, 123], [750, -250, 100], [1000, 400]).unwrap();
    assert_eq!(
        f.metadata().vertical_metrics.source,
        VerticalMetricSource::Typographic
    );
    let f = face(4, 0, [0, 0, 123], [0, 0, 0], [40000, 35000]).unwrap();
    let m = f.metadata().vertical_metrics;
    assert_eq!(m.source, VerticalMetricSource::Windows);
    assert_eq!([m.ascender, m.descender, m.line_gap], [40000, -35000, 0]);
    let f = face(4, 0, [0, 0, 0], [0, 0, 0], [32768, 32768]).unwrap();
    assert_eq!(f.metadata().vertical_metrics.descender, -32768);
}
#[test]
fn malformed_authoritative_metrics_and_invalid_normalization_return_owned_errors() {
    assert!(matches!(
        face(4, 0x80, [900, -300, 50], [0, 100, 0], [1000, 400]),
        Err(FontError::InvalidFont)
    ));
    let m = FontVerticalMetrics {
        units_per_em: 1000,
        ascender: 800,
        descender: -200,
        line_gap: -50,
        source: VerticalMetricSource::Hhea,
    };
    assert_eq!(m.at_size(10.0).unwrap().line_gap, 0.0);
    assert_eq!(
        FontVerticalMetrics {
            units_per_em: 0,
            ..m
        }
        .at_size(10.0),
        Err(FontError::InvalidRequest)
    );
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(m.at_size(size), Err(FontError::InvalidRequest));
    }
}

#[test]
fn empty_source_without_fonts_preserves_empty_document_conversion() {
    let report =
        o2p_fonts::shape(&o2p_fonts::FontBook::new(), "", &Default::default(), 20.0).unwrap();
    assert!(report.text.runs.is_empty() && report.warnings.is_empty());
    assert_eq!(report.metrics.height(), 0.0);
}
