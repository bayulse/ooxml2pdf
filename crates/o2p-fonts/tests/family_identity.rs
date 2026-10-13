use o2p_fonts::*;
use std::{path::PathBuf, sync::Arc};

fn fixture() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/O2PTestSans-Regular.ttf"),
    )
    .unwrap()
}

fn table_record(bytes: &[u8], tag: &[u8; 4]) -> usize {
    (0..u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize)
        .map(|i| 12 + i * 16)
        .find(|&i| &bytes[i..i + 4] == tag)
        .unwrap()
}

// Original OFL outlines, synthetic Unicode name records and OS/2 styles.
// No proprietary font data, parser dependency, paths, or fixture files added.
fn named(records: &[(u16, u16, &str)], weight: u16, italic: bool) -> Arc<[u8]> {
    let mut bytes = fixture();
    let mut strings = Vec::new();
    let mut names = Vec::new();
    names.extend_from_slice(&0u16.to_be_bytes());
    names.extend_from_slice(&(records.len() as u16).to_be_bytes());
    names.extend_from_slice(&(6u16 + records.len() as u16 * 12).to_be_bytes());
    for &(id, language, value) in records {
        let text: Vec<u8> = value.encode_utf16().flat_map(u16::to_be_bytes).collect();
        for n in [
            3u16,
            1,
            language,
            id,
            text.len() as u16,
            strings.len() as u16,
        ] {
            names.extend_from_slice(&n.to_be_bytes());
        }
        strings.extend(text);
    }
    names.extend(strings);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    let offset = bytes.len();
    bytes.extend(&names);
    let record = table_record(&bytes, b"name");
    bytes[record + 8..record + 12].copy_from_slice(&(offset as u32).to_be_bytes());
    bytes[record + 12..record + 16].copy_from_slice(&(names.len() as u32).to_be_bytes());
    let record = table_record(&bytes, b"OS/2");
    let offset = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
    bytes[offset + 4..offset + 6].copy_from_slice(&weight.to_be_bytes());
    let selection = u16::from(italic)
        | if weight >= 700 {
            32
        } else if !italic {
            64
        } else {
            0
        };
    bytes[offset + 62..offset + 64].copy_from_slice(&selection.to_be_bytes());
    bytes.into()
}

fn face(family: &str, group: &str, ps: &str, weight: u16, italic: bool) -> Arc<[u8]> {
    let style = match (weight >= 700, italic) {
        (false, false) => "Regular",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (true, true) => "Bold Italic",
    };
    named(
        &[
            (1, 0x409, family),
            (2, 0x409, style),
            (16, 0x409, group),
            (17, 0x409, style),
            (4, 0x409, ps),
            (6, 0x409, ps),
        ],
        weight,
        italic,
    )
}

fn request(family: &str, weight: u16, italic: bool) -> FontRequest {
    FontRequest {
        family: family.into(),
        weight,
        italic,
    }
}

#[test]
fn canonical_identity_beats_overlapping_aliases_for_all_four_styles_and_both_orders() {
    let mut files = Vec::new();
    for family in ["Example", "Example Display"] {
        for (weight, italic, suffix) in [
            (400, false, "Regular"),
            (700, false, "Bold"),
            (400, true, "Italic"),
            (700, true, "BoldItalic"),
        ] {
            files.push(face(
                family,
                "Example",
                &format!("{family}-{suffix}"),
                weight,
                italic,
            ));
        }
    }
    for reversed in [false, true] {
        let mut files = files.clone();
        if reversed {
            files.reverse();
        }
        let mut book = FontBook::new();
        for bytes in files {
            book.register_bytes(bytes).unwrap();
        }
        for family in ["Example", "Example Display"] {
            for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
                let req = request(&format!(" {} ", family.to_uppercase()), weight, italic);
                let selected = book.resolve(&req, "Résumé ffi").unwrap();
                let m = selected.font.metadata();
                assert_eq!(m.family, family);
                assert_eq!((m.weight, m.italic), (weight, italic));
                assert!(selected.warnings.is_empty(), "{:?}", selected.warnings);
                let diagnostic = selected.diagnostic(&req);
                assert_eq!(diagnostic.requested, req);
                assert_eq!(diagnostic.resolved_family, family);
                assert_eq!(diagnostic.postscript_name, m.postscript_name);
                assert_eq!(diagnostic.resolved_subfamily, m.subfamily);
                assert_eq!(
                    (diagnostic.resolved_weight, diagnostic.resolved_italic),
                    (weight, italic)
                );
                let shaped = shape(&book, "Résumé ffi", &req, 44.0).unwrap();
                assert!(shaped.warnings.is_empty());
                assert!(Arc::ptr_eq(
                    &shaped.text.runs[0].font,
                    selected.font.resource()
                ));
            }
        }
    }
}

#[test]
fn canonical_family_precedes_better_style_in_an_alias_family() {
    let mut book = FontBook::new();
    book.register_bytes(face(
        "Example Display",
        "Example",
        "DisplayBold",
        700,
        false,
    ))
    .unwrap();
    book.register_bytes(face("Example", "Example", "ExampleRegular", 400, false))
        .unwrap();
    let selected = book.resolve(&request("Example", 700, false), "A").unwrap();
    assert_eq!(selected.font.metadata().family, "Example");
    assert_eq!(selected.font.metadata().weight, 400);
    assert!(matches!(
        selected.warnings.as_slice(),
        [FontWarning::Substitution { .. }]
    ));
}

#[test]
fn metadata_aliases_are_used_when_canonical_identity_is_absent() {
    let mut book = FontBook::new();
    book.register_bytes(face("Example Display", "Example", "Display", 400, false))
        .unwrap();
    let selected = book
        .resolve(&request(" eXAMPLE ", 400, false), "A")
        .unwrap();
    assert_eq!(selected.font.metadata().family, "Example Display");
    assert!(
        !selected
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::MissingFont { .. }))
    );
    assert!(
        matches!(selected.warnings.as_slice(), [FontWarning::Substitution {
        selected_family, .. }] if selected_family == "Example Display")
    );
}

#[test]
fn related_names_full_names_and_postscript_names_do_not_implicitly_match() {
    let mut book = FontBook::new();
    book.register_bytes(face(
        "Example Display",
        "Example",
        "ExampleDisplay-Regular",
        400,
        false,
    ))
    .unwrap();
    for family in ["Example Text", "Example Narrow", "ExampleDisplay-Regular"] {
        assert_eq!(
            book.resolve(&request(family, 400, false), "A")
                .err()
                .unwrap(),
            FontError::NoFonts
        );
    }
    // Explicit fallback remains eligible, and reports both names.
    book.set_fallback_families(vec!["Example Display".into()]);
    let selected = book
        .resolve(&request("Example Narrow", 400, false), "A")
        .unwrap();
    assert!(selected.warnings.contains(&FontWarning::MissingFont {
        family: "Example Narrow".into()
    }));
    assert!(selected.warnings.iter().any(|w| matches!(w, FontWarning::Substitution {
        requested, selected_family, .. } if requested.family == "Example Narrow" && selected_family == "Example Display")));
    book.set_family_alias("Example Display".into(), vec!["Example".into()]);
    assert!(
        book.resolve(&request("Example Display", 400, false), "A")
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn localized_names_and_subfamilies_are_retained_with_stable_preference() {
    let records = [
        (16, 0x409, "Example"),
        (17, 0x409, "Display"),
        (1, 0x411, "例 ディスプレイ"),
        (1, 0x409, "Example Display"),
        (2, 0x409, "Regular"),
        (4, 0x409, "Example Display Regular"),
        (6, 0x409, "ExampleDisplay"),
    ];
    let mut reversed = records;
    reversed.reverse();
    let first = FontFace::from_bytes(named(&records, 400, false)).unwrap();
    let second = FontFace::from_bytes(named(&reversed, 400, false)).unwrap();
    assert_eq!(first.metadata(), second.metadata());
    let m = first.metadata();
    assert_eq!(m.family, "Example Display");
    assert_eq!(m.subfamily, "Regular");
    assert_eq!(m.full_name.as_deref(), Some("Example Display Regular"));
    assert_eq!(m.postscript_name, "ExampleDisplay");
    assert_eq!(m.names.len(), records.len());
    assert!(m.family_aliases.contains(&"例 ディスプレイ".into()));
    let mut book = FontBook::new();
    book.register_bytes(named(&records, 400, false)).unwrap();
    assert_eq!(
        book.resolve(&request("例 ディスプレイ", 400, false), "A")
            .unwrap()
            .font
            .metadata()
            .family,
        "Example Display"
    );
}

#[test]
fn wws_identity_and_typographic_only_fonts_follow_documented_precedence() {
    let mut records = vec![
        (1, 0x409, "Example Light"),
        (2, 0x409, "Regular"),
        (16, 0x409, "Example"),
        (17, 0x409, "Display Light"),
        (21, 0x409, "Example Display"),
        (22, 0x409, "Light"),
    ];
    let f = FontFace::from_bytes(named(&records, 300, false)).unwrap();
    assert_eq!(f.metadata().family, "Example Display");
    assert_eq!(f.metadata().subfamily, "Light");
    assert_eq!(
        f.metadata().family_aliases,
        ["Example", "Example Display", "Example Light"]
    );
    records.retain(|(id, _, _)| matches!(id, 16 | 17));
    let f = FontFace::from_bytes(named(&records, 300, false)).unwrap();
    assert_eq!(f.metadata().family, "Example");
    assert_eq!(f.metadata().subfamily, "Display Light");
}

#[test]
fn invalid_empty_and_oversized_name_records_do_not_hide_valid_names() {
    let oversized = "A".repeat(257);
    let bytes = named(
        &[
            (1, 0x409, ""),
            (1, 0x409, "  "),
            (1, 0x409, &oversized),
            (1, 0x409, "Valid"),
            (6, 0x409, "ValidRegular"),
        ],
        400,
        false,
    );
    let f = FontFace::from_bytes(bytes).unwrap();
    assert_eq!(f.metadata().family, "Valid");
    assert_eq!(f.metadata().names.len(), 2);
    let mut bytes = named(&[(1, 0x409, "Bad"), (1, 0x409, "Valid")], 400, false).to_vec();
    let record = table_record(&bytes, b"name");
    let offset = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
    let strings = u16::from_be_bytes(bytes[offset + 4..offset + 6].try_into().unwrap()) as usize;
    bytes[offset + strings..offset + strings + 2].copy_from_slice(&0xd800u16.to_be_bytes());
    let f = FontFace::from_bytes(bytes.into()).unwrap();
    assert_eq!(f.metadata().family, "Valid");
    assert_eq!(f.metadata().names.len(), 1);
}

#[test]
fn same_strength_ties_use_family_then_postscript_then_bytes_in_both_orders() {
    let a = face("Example Display", "Alias", "A-Display", 400, false);
    let b = face("Example Text", "Alias", "B-Text", 400, false);
    let c = face("Example Display", "Alias", "Z-Display", 400, false);
    for files in [vec![a.clone(), b.clone(), c.clone()], vec![c, b, a]] {
        let mut book = FontBook::new();
        for bytes in files {
            book.register_bytes(bytes).unwrap();
        }
        assert_eq!(
            book.resolve(&request("Alias", 400, false), "A")
                .unwrap()
                .font
                .metadata()
                .postscript_name,
            "A-Display"
        );
    }
    let a = face("Example", "Example", "Same", 400, false);
    let b = named(
        &[
            (1, 0x409, "Example"),
            (2, 0x409, "Regular"),
            (6, 0x409, "Same"),
            (4, 0x409, "Different Full Name"),
        ],
        400,
        false,
    );
    let expected = a.clone().min(b.clone());
    for files in [vec![a.clone(), b.clone()], vec![b, a]] {
        let mut book = FontBook::new();
        for bytes in files {
            book.register_bytes(bytes).unwrap();
        }
        let selected = book.resolve(&request("Example", 400, false), "A").unwrap();
        assert_eq!(selected.font.resource().bytes, expected);
    }
}

#[test]
fn glyph_coverage_can_fall_back_without_splitting_unicode_clusters() {
    let mut book = FontBook::new();
    book.register_bytes(face("Example Display", "Example", "Display", 400, false))
        .unwrap();
    let japanese = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/O2PTestJapanese-Regular.ttf"),
    )
    .unwrap();
    book.register_bytes(japanese.into()).unwrap();
    book.set_fallback_families(vec!["O2P Test Japanese".into()]);
    let selected = book
        .resolve(&request("Example Display", 400, false), "日本語")
        .unwrap();
    assert_eq!(selected.font.metadata().family, "O2P Test Japanese");
    let shaped = shape(
        &book,
        "Résumé a\u{301}\u{323} ffi 日本語",
        &request("Example Display", 400, false),
        18.0,
    )
    .unwrap();
    assert!(
        !shaped
            .warnings
            .iter()
            .any(|w| matches!(w, FontWarning::MissingGlyph { .. }))
    );
    assert!(
        shaped
            .text
            .runs
            .iter()
            .all(|r| r.glyphs.iter().all(|g| g.id > 0))
    );
}
