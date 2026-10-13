use o2p_fonts::*;
use std::{path::PathBuf, sync::Arc};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(name)
}
fn book() -> FontBook {
    let mut book = FontBook::new();
    book.register_directory(fixture("")).unwrap();
    book.set_fallback_families(vec![
        "O2P Test Sans".into(),
        "O2P Test Arabic".into(),
        "O2P Test Japanese".into(),
    ]);
    book
}
fn request() -> FontRequest {
    FontRequest {
        family: "O2P Test Sans".into(),
        ..FontRequest::default()
    }
}

#[test]
fn registration_metadata_file_bytes_directory_and_shared_ownership() {
    let bytes: Arc<[u8]> = std::fs::read(fixture("O2PTestSans-Regular.ttf"))
        .unwrap()
        .into();
    let mut registry = FontBook::new();
    let font = registry.register_bytes(bytes.clone()).unwrap();
    assert!(Arc::ptr_eq(&font.resource().bytes, &bytes));
    assert_eq!(font.metadata().family, "O2P Test Sans");
    assert_eq!(font.metadata().weight, 400);
    assert!(!font.metadata().italic);
    assert!(font.metadata().ascent > 0 && font.metadata().descent < 0);
    assert!(font.glyph_id('é').is_some());
    assert!(font.glyph_advance(font.glyph_id('A').unwrap()).unwrap() > 0);
    let by_file = registry
        .register_file(fixture("O2PTestSans-Regular.ttf"))
        .unwrap();
    assert_eq!(font.metadata(), by_file.metadata());
    assert_eq!(book().fonts().len(), 4);
}

#[test]
fn family_style_matching_aliases_fallback_and_warnings_are_deterministic() {
    let mut registry = book();
    let selection = registry
        .resolve(
            &FontRequest {
                family: " o2p TEST sans ".into(),
                ..request()
            },
            "Résumé",
        )
        .unwrap();
    assert!(selection.warnings.is_empty());
    let styled = FontRequest {
        weight: 700,
        italic: true,
        ..request()
    };
    let font = registry.resolve(&styled, "Bold").unwrap();
    assert_eq!(font.font.metadata().weight, 700);
    assert!(font.font.metadata().italic);
    assert!(font.warnings.is_empty());
    let unavailable_style = registry
        .resolve(
            &FontRequest {
                italic: true,
                ..request()
            },
            "Italic",
        )
        .unwrap();
    assert!(
        unavailable_style
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::Substitution { .. }))
    );
    registry.set_family_alias("Missing Office Family".into(), vec!["O2P Test Sans".into()]);
    let requested = FontRequest {
        family: "Missing Office Family".into(),
        ..request()
    };
    let alias = registry.resolve(&requested, "text").unwrap();
    assert_eq!(alias.font.metadata().family, "O2P Test Sans");
    assert!(
        alias
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::MissingFont { .. }))
    );
    assert!(
        alias
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::Substitution { .. }))
    );
    let fallback = registry.resolve(&request(), "日本語").unwrap();
    assert_eq!(fallback.font.metadata().family, "O2P Test Japanese");
    assert!(
        fallback
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::Substitution { .. }))
    );
    assert!(
        !fallback
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::MissingGlyph { .. }))
    );
    let missing = registry.resolve(&request(), "😀").unwrap();
    assert!(
        missing
            .warnings
            .contains(&FontWarning::MissingGlyph { character: '😀' })
    );
    assert!(matches!(
        FontBook::new().resolve(&request(), "A"),
        Err(FontError::NoFonts)
    ));
}

#[test]
fn unicode_clusters_ligatures_offsets_and_visual_glyph_order() {
    let registry = book();
    for source in [
        "ASCII",
        "Résumé",
        "Ελληνικά",
        "Привет",
        "日本語",
        "مرحبا",
        "Latin 日本語 مرحبا",
    ] {
        let shaped = shape(&registry, source, &request(), 18.0).unwrap();
        assert_eq!(shaped.text.source, source);
        assert!(shaped.text.advance.x > 0.0);
        assert!(
            !shaped
                .warnings
                .iter()
                .any(|w| matches!(w, FontWarning::MissingGlyph { .. })),
            "{source}: {:?}",
            shaped.warnings
        );
        for run in &shaped.text.runs {
            for glyph in &run.glyphs {
                assert!(glyph.id > 0);
                assert!(source.get(glyph.source.clone()).is_some());
            }
        }
        assert_eq!(shaped, shape(&registry, source, &request(), 18.0).unwrap());
    }
    let ffi = shape(&registry, "ffi", &request(), 18.0).unwrap();
    assert_eq!(ffi.text.runs[0].glyphs.len(), 1);
    assert_eq!(ffi.text.runs[0].glyphs[0].source, 0..3);
    let combining = shape(&registry, "a\u{301}\u{323}", &request(), 18.0).unwrap();
    assert!(
        combining.text.runs[0].glyphs.len() < 3
            || combining.text.runs[0]
                .glyphs
                .iter()
                .any(|g| g.offset.x != 0.0 || g.offset.y != 0.0)
    );
    assert!(
        combining.text.runs[0]
            .glyphs
            .iter()
            .all(|g| g.source == (0..5))
    );
    let ascii = shape(&registry, "ABC", &request(), 18.0).unwrap();
    assert_eq!(
        ascii.text.runs[0]
            .glyphs
            .iter()
            .map(|g| g.source.start)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let arabic = shape(&registry, "مرحبا", &request(), 18.0).unwrap();
    assert!(
        arabic.text.runs[0]
            .glyphs
            .windows(2)
            .all(|w| w[0].source.start >= w[1].source.start)
    );
}

#[test]
fn malformed_unsupported_restricted_fonts_and_limits_return_owned_errors() {
    let original = std::fs::read(fixture("O2PTestSans-Regular.ttf")).unwrap();
    for size in [0, 1, 4, 12, original.len() / 2] {
        assert!(FontFace::from_bytes(Arc::from(&original[..size])).is_err());
    }
    for tag in [b"CFF ", b"CFF2", b"fvar", b"COLR", b"CPAL", b"SVG "] {
        let mut bytes = original.clone();
        let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let offset = (0..count)
            .map(|i| 12 + i * 16)
            .find(|&i| &bytes[i..i + 4] == b"post")
            .unwrap();
        bytes[offset..offset + 4].copy_from_slice(tag);
        assert!(matches!(
            FontFace::from_bytes(bytes.into()),
            Err(FontError::UnsupportedTechnology(_))
        ));
    }
    let mut restricted = original.clone();
    let count = u16::from_be_bytes([restricted[4], restricted[5]]) as usize;
    let record = (0..count)
        .map(|i| 12 + i * 16)
        .find(|&i| &restricted[i..i + 4] == b"OS/2")
        .unwrap();
    let start =
        u32::from_be_bytes(restricted[record + 8..record + 12].try_into().unwrap()) as usize;
    restricted[start + 8..start + 10].copy_from_slice(&2u16.to_be_bytes());
    assert!(matches!(
        FontFace::from_bytes(restricted.into()),
        Err(FontError::EmbeddingRestricted)
    ));
    assert!(matches!(
        FontFace::from_bytes(vec![0; MAX_FONT_BYTES + 1].into()),
        Err(FontError::LimitExceeded(_))
    ));
    let registry = book();
    assert!(shape(&registry, &"A".repeat(MAX_TEXT_BYTES + 1), &request(), 12.0).is_err());
    assert!(shape(&registry, "A\nB", &request(), 12.0).is_err());
    assert!(shape(&registry, "A", &request(), f32::NAN).is_err());
    // Mutation/truncation smoke tests exercise untrusted font entry points.
    for position in (0..original.len()).step_by(991) {
        let mut bytes = original.clone();
        bytes[position] ^= 0xff;
        let _ = FontFace::from_bytes(bytes.into());
    }
}

// Replace an optional table tag in memory and retain sorted sfnt records.
// The replacement data is intentionally not a valid bitmap table: registration
// must use validated outlines rather than parsing/rendering auxiliary strikes.
fn with_auxiliary_bitmap(original: &[u8], tag: &[u8; 4]) -> Vec<u8> {
    let mut bytes = original.to_vec();
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let end = 12 + count * 16;
    let mut records: Vec<_> = bytes[12..end]
        .chunks_exact(16)
        .map(<[u8]>::to_vec)
        .collect();
    let record = records.iter_mut().find(|r| &r[..4] == b"post").unwrap();
    record[..4].copy_from_slice(tag);
    records.sort_by(|a, b| a[..4].cmp(&b[..4]));
    for (dest, record) in bytes[12..end].chunks_exact_mut(16).zip(records) {
        dest.copy_from_slice(&record);
    }
    bytes
}

#[test]
fn auxiliary_bitmap_records_preserve_outline_registration_and_shaping() {
    let original = std::fs::read(fixture("O2PTestSans-Regular.ttf")).unwrap();
    let baseline = FontFace::from_bytes(original.clone().into()).unwrap();
    let mut baseline_book = FontBook::new();
    baseline_book
        .register_bytes(original.clone().into())
        .unwrap();
    let baseline_shape = shape(&baseline_book, "Résumé ffi", &request(), 18.0).unwrap();
    for tag in [
        b"EBDT", b"EBLC", b"CBDT", b"CBLC", b"sbix", b"bdat", b"bloc",
    ] {
        let mut registry = FontBook::new();
        let font = registry
            .register_bytes(with_auxiliary_bitmap(&original, tag).into())
            .unwrap();
        assert_eq!(font.metadata(), baseline.metadata(), "{tag:?}");
        let shaped = shape(&registry, "Résumé ffi", &request(), 18.0).unwrap();
        assert_eq!(shaped.warnings, baseline_shape.warnings);
        assert_eq!(shaped.text.advance, baseline_shape.text.advance);
        assert_eq!(
            shaped.text.runs[0].glyphs,
            baseline_shape.text.runs[0].glyphs
        );
    }
}

#[test]
fn auxiliary_bitmaps_do_not_bypass_outline_validation_or_embedding_rights() {
    let original = std::fs::read(fixture("O2PTestSans-Regular.ttf")).unwrap();
    for tag in [b"glyf", b"loca", b"hmtx", b"cmap", b"OS/2"] {
        let mut bytes = with_auxiliary_bitmap(&original, b"EBDT");
        let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let record = (0..count)
            .map(|i| 12 + i * 16)
            .find(|&i| &bytes[i..i + 4] == tag)
            .unwrap();
        // Existing record, unusable contents (outside the byte buffer).
        bytes[record + 8..record + 12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(
            matches!(
                FontFace::from_bytes(bytes.into()),
                Err(FontError::InvalidFont)
            ),
            "{tag:?}"
        );
    }
    for rights in [2u16, 0x0200] {
        let mut bytes = with_auxiliary_bitmap(&original, b"EBDT");
        let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let record = (0..count)
            .map(|i| 12 + i * 16)
            .find(|&i| &bytes[i..i + 4] == b"OS/2")
            .unwrap();
        let start = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
        bytes[start + 8..start + 10].copy_from_slice(&rights.to_be_bytes());
        assert!(matches!(
            FontFace::from_bytes(bytes.into()),
            Err(FontError::EmbeddingRestricted)
        ));
    }
}

#[test]
fn registered_and_directory_font_limits_remain_256() {
    let bytes: Arc<[u8]> = std::fs::read(fixture("O2PTestJapanese-Regular.ttf"))
        .unwrap()
        .into();
    let mut registry = FontBook::new();
    for _ in 0..256 {
        registry.register_bytes(bytes.clone()).unwrap();
    }
    assert_eq!(
        registry.register_bytes(bytes).unwrap_err(),
        FontError::LimitExceeded("registered fonts")
    );
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = Directory(
        std::env::temp_dir().join(format!("o2p-font-limit-{}-{unique}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).unwrap();
    for index in 0..257 {
        std::fs::write(directory.0.join(format!("{index}.ttf")), []).unwrap();
    }
    let mut registry = FontBook::new();
    assert_eq!(
        registry.register_directory(&directory.0).unwrap_err(),
        FontError::LimitExceeded("directory fonts")
    );
    assert!(registry.fonts().is_empty());
}
