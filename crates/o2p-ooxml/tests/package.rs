mod support;
use o2p_ooxml::{ContentType, OoxmlPackage, PackageError, RelationshipTarget, SecurityLimits};
use std::path::Path;
use support::{minimal, relation, rels, zip};
fn fixture(name: &str) -> OoxmlPackage {
    OoxmlPackage::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pptx")
            .join(name),
        SecurityLimits::default(),
    )
    .unwrap()
}
fn open(parts: &[(String, Vec<u8>)]) -> Result<OoxmlPackage, PackageError> {
    OoxmlPackage::from_bytes(&zip(parts, true, false), SecurityLimits::default())
}
fn replace(parts: &mut [(String, Vec<u8>)], name: &str, bytes: Vec<u8>) {
    parts.iter_mut().find(|(n, _)| n == name).unwrap().1 = bytes;
}
fn central(bytes: &[u8]) -> usize {
    bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap()
}
fn u32w(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn detects_pptx_and_enumerates_slides() {
    let package = fixture("minimal.pptx");
    assert!(package.is_pptx());
    let inspection = package.inspect_pptx().unwrap();
    assert_eq!(inspection.presentation_part, "ppt/presentation.xml");
    assert_eq!(inspection.slides, ["ppt/slides/slide1.xml"]);
    assert_eq!(inspection.slide_count(), 1);
    assert_eq!(inspection.media_count(), 0);
    assert_eq!(
        fixture("three-slides.pptx").inspect_pptx().unwrap().slides,
        [
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
            "ppt/slides/slide3.xml"
        ]
    );
    assert!(
        package
            .part("ppt/./slides/slide1.xml")
            .unwrap()
            .bytes()
            .starts_with(b"<p:sld")
    );
    assert!(package.part("missing").is_err());
    assert!(package.part("../escape").is_err());
    assert!(package.relationships(Some("missing")).is_err());
    assert!(
        package
            .relationships(Some("ppt/slides/slide1.xml"))
            .unwrap()
            .is_empty()
    );
}
#[test]
fn media_and_part_relationships_are_resolved_offline() {
    let package = fixture("one-image.pptx");
    let inspection = package.inspect_pptx().unwrap();
    assert_eq!(inspection.media_count(), 1);
    assert_eq!(inspection.media, ["ppt/media/image1.png"]);
    let part = package.part(&inspection.media[0]).unwrap();
    assert_eq!(part.name(), "ppt/media/image1.png");
    assert_eq!(part.content_type(), "image/png");
    assert!(part.bytes().starts_with(b"\x89PNG"));
    let relations = package
        .relationships(Some("ppt/slides/slide1.xml"))
        .unwrap();
    assert_eq!(
        relations[0].target,
        RelationshipTarget::Internal("ppt/media/image1.png".into())
    );
    assert_eq!(
        relations[1].target,
        RelationshipTarget::External("https://example.invalid/never-fetch".into())
    );
    assert_eq!(package.relationships(None).unwrap()[0].id, "rOffice");
}
#[test]
fn content_type_overrides_and_case_insensitive_extensions() {
    let mut parts = minimal(1, false);
    parts.push(("extra.XML".into(), b"unused".to_vec()));
    let package = open(&parts).unwrap();
    assert_eq!(
        package.part("extra.XML").unwrap().content_type(),
        "application/xml"
    );
    assert!(
        package
            .part("ppt/slides/slide1.xml")
            .unwrap()
            .content_type()
            .ends_with("slide+xml")
    );
    assert!(
        package
            .content_types()
            .iter()
            .any(|t| matches!(t, ContentType::Default { extension, .. } if extension == "xml"))
    );
}
#[test]
fn inspection_uses_metadata_not_filename_or_conventional_main_path() {
    let mut parts = minimal(1, false);
    replace(
        &mut parts,
        "_rels/.rels",
        rels(&relation(
            "rOffice",
            "notOfficeDocument",
            "ppt/presentation.xml",
        )),
    );
    let package = open(&parts).unwrap();
    assert!(!package.is_pptx());
    assert!(matches!(package.inspect_pptx(), Err(PackageError::NotPptx)));
    replace(
        &mut parts,
        "_rels/.rels",
        rels(&relation("rOffice", "officeDocument", "custom/deck.xml")),
    );
    for (name, data) in &mut parts {
        if name == "ppt/presentation.xml" {
            *name = "custom/deck.xml".into();
        } else if name == "ppt/_rels/presentation.xml.rels" {
            *name = "custom/_rels/deck.xml.rels".into();
            *data = rels(&relation("rId1", "slide", "../ppt/slides/slide1.xml"));
        } else if name == "[Content_Types].xml" {
            *data = String::from_utf8(data.clone())
                .unwrap()
                .replace("/ppt/presentation.xml", "/custom/deck.xml")
                .into_bytes();
        }
    }
    let package = open(&parts).unwrap();
    assert!(package.is_pptx());
    assert_eq!(
        package.inspect_pptx().unwrap().presentation_part,
        "custom/deck.xml"
    );
}
#[test]
fn strict_office_relationship_uri_is_recognized() {
    let mut parts = minimal(1, false);
    let strict = String::from_utf8(rels(&relation(
        "rOffice",
        "officeDocument",
        "ppt/presentation.xml",
    )))
    .unwrap()
    .replace(
        support::OFFICE_RELS,
        "http://purl.oclc.org/ooxml/officeDocument/relationships",
    );
    replace(&mut parts, "_rels/.rels", strict.into_bytes());
    assert!(open(&parts).unwrap().is_pptx());
}
#[test]
fn malicious_and_malformed_fixtures_return_owned_errors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pptx");
    for name in [
        "malformed-relationships.pptx",
        "malformed-xml.pptx",
        "path-traversal.pptx",
        "target-traversal.pptx",
        "compression-abuse.pptx",
    ] {
        assert!(
            OoxmlPackage::open(dir.join(name), SecurityLimits::default()).is_err(),
            "{name}"
        );
    }
    let error =
        OoxmlPackage::open(dir.join("does-not-exist.pptx"), SecurityLimits::default()).unwrap_err();
    assert!(matches!(error, PackageError::Io(_)));
}
#[test]
fn limits_are_configurable_and_boundary_is_inclusive() {
    let bytes = zip(&minimal(1, false), false, false);
    let limits = SecurityLimits {
        max_input_size: bytes.len() as u64,
        max_zip_entries: 5,
        ..SecurityLimits::default()
    };
    OoxmlPackage::from_bytes(&bytes, limits).unwrap();
    for (limits, expected) in [
        (
            SecurityLimits {
                max_input_size: bytes.len() as u64 - 1,
                ..limits
            },
            "max_input_size",
        ),
        (
            SecurityLimits {
                max_zip_entries: 4,
                ..limits
            },
            "max_zip_entries",
        ),
        (
            SecurityLimits {
                max_total_uncompressed_size: 1,
                ..limits
            },
            "max_total_uncompressed_size",
        ),
        (
            SecurityLimits {
                max_part_size: 1,
                ..limits
            },
            "max_part_size",
        ),
        (
            SecurityLimits {
                max_compression_ratio: 0,
                ..limits
            },
            "max_compression_ratio",
        ),
        (
            SecurityLimits {
                max_xml_size: 1,
                ..limits
            },
            "max_xml_size",
        ),
        (
            SecurityLimits {
                max_relationships: 1,
                ..limits
            },
            "max_relationships",
        ),
    ] {
        assert_eq!(
            OoxmlPackage::from_bytes(&bytes, limits).unwrap_err(),
            PackageError::LimitExceeded { limit: expected }
        );
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pptx");
    let bytes = std::fs::read(dir.join("entry-limit.pptx")).unwrap();
    assert!(matches!(
        OoxmlPackage::from_bytes(
            &bytes,
            SecurityLimits {
                max_zip_entries: 20,
                ..SecurityLimits::default()
            }
        ),
        Err(PackageError::LimitExceeded {
            limit: "max_zip_entries"
        })
    ));
    OoxmlPackage::from_bytes(
        &bytes,
        SecurityLimits {
            max_zip_entries: 21,
            ..SecurityLimits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        OoxmlPackage::open(
            dir.join("minimal.pptx"),
            SecurityLimits {
                max_input_size: 1,
                ..SecurityLimits::default()
            }
        ),
        Err(PackageError::LimitExceeded {
            limit: "max_input_size"
        })
    ));
}
#[test]
fn zip_stored_deflate_directories_and_data_descriptors() {
    let mut parts = minimal(1, true);
    parts.push(("empty/".into(), vec![]));
    for deflate in [false, true] {
        for descriptor in [false, true] {
            let package = OoxmlPackage::from_bytes(
                &zip(&parts, deflate, descriptor),
                SecurityLimits::default(),
            )
            .unwrap();
            assert_eq!(package.inspect_pptx().unwrap().media_count(), 1);
            assert!(package.parts().all(|p| !p.name().ends_with('/')));
        }
    }
}
#[test]
fn missing_metadata_targets_and_orphan_relationships_fail() {
    for removed in [
        "[Content_Types].xml",
        "_rels/.rels",
        "ppt/presentation.xml",
        "ppt/slides/slide1.xml",
    ] {
        let mut parts = minimal(1, false);
        parts.retain(|(name, _)| name != removed);
        assert!(matches!(open(&parts), Err(PackageError::MissingPart(_))));
    }
    let mut parts = minimal(1, false);
    replace(
        &mut parts,
        "ppt/_rels/presentation.xml.rels",
        rels(&relation("missing", "slide", "slides/missing.xml")),
    );
    assert!(matches!(open(&parts), Err(PackageError::MissingPart(_))));
    let mut parts = minimal(1, false);
    parts.push(("ppt/_rels/orphan.xml.rels".into(), rels("")));
    assert!(matches!(open(&parts), Err(PackageError::MissingPart(_))));
    let mut parts = minimal(1, false);
    parts.push(("unknown.foo".into(), vec![]));
    assert!(matches!(
        open(&parts),
        Err(PackageError::InvalidMetadata(_))
    ));
}
#[test]
fn all_relationship_parts_are_validated_even_if_not_reachable() {
    let mut parts = minimal(1, false);
    parts.push(("unused.xml".into(), vec![]));
    parts.push((
        "_rels/unused.xml.rels".into(),
        rels("<Relationship Id='bad' Target='foo'/>"),
    ));
    assert!(matches!(
        open(&parts),
        Err(PackageError::InvalidMetadata(_))
    ));
}
#[test]
fn duplicate_parts_and_unsafe_zip_names_are_rejected() {
    let mut parts = minimal(1, false);
    parts.push(parts[0].clone());
    assert!(matches!(open(&parts), Err(PackageError::DuplicatePart(_))));
    let mut parts = minimal(1, false);
    parts.push((
        "ppt/./slides/slide1.xml".into(),
        b"duplicate alias".to_vec(),
    ));
    assert!(matches!(open(&parts), Err(PackageError::DuplicatePart(_))));
    for path in [
        "../escape.dat",
        "/absolute.dat",
        "C:/absolute.dat",
        "ppt/../../escape.dat",
        "ppt/%2e%2e/escape.dat",
        "ppt\\escape.dat",
    ] {
        let mut parts = minimal(1, false);
        parts.push((path.into(), vec![]));
        assert!(
            matches!(open(&parts), Err(PackageError::InvalidPath(_))),
            "{path}"
        );
    }
}
#[test]
fn crc_size_and_directory_corruption_fail() {
    let original = zip(&minimal(1, false), false, false);
    let cd = central(&original);
    for offset in [
        30 + "ppt/slides/slide1.xml".len(),
        cd + 16,
        cd + 20,
        cd + 24,
        cd + 42,
    ] {
        let mut bytes = original.clone();
        bytes[offset] ^= 1;
        assert!(OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()).is_err());
    }
    for len in 0..original.len() {
        assert!(
            OoxmlPackage::from_bytes(&original[..len], SecurityLimits::default()).is_err(),
            "truncated to {len}"
        );
    }
}
#[test]
fn forged_deflate_size_is_bounded_by_actual_output() {
    let mut bytes = zip(&minimal(1, false), true, false);
    let cd = central(&bytes);
    // Forge both declared sizes; decompression must stop beyond this tiny budget.
    u32w(&mut bytes, 22, 1);
    u32w(&mut bytes, cd + 24, 1);
    assert!(matches!(
        OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
        Err(PackageError::InvalidZip(_))
    ));
}
#[test]
fn unsupported_zip_features_fail_explicitly() {
    let original = zip(&minimal(1, false), false, false);
    let cd = central(&original);
    for (local, central_offset, value) in [(6, 8, 1u16), (8, 10, 99)] {
        let mut bytes = original.clone();
        bytes[local..local + 2].copy_from_slice(&value.to_le_bytes());
        bytes[cd + central_offset..cd + central_offset + 2].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
            Err(PackageError::Unsupported(_))
        ));
    }
    let mut bytes = original.clone();
    u32w(&mut bytes, cd + 38, 0xa000 << 16);
    assert!(matches!(
        OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
        Err(PackageError::Unsupported(_))
    ));
    let mut bytes = original.clone();
    let end = bytes.len() - 22;
    bytes[end + 4] = 1;
    assert!(matches!(
        OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
        Err(PackageError::Unsupported(_))
    ));
    let mut bytes = original;
    let end = bytes.len() - 22;
    bytes[end + 8..end + 12].fill(255);
    assert!(matches!(
        OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
        Err(PackageError::Unsupported(_))
    ));
}
#[test]
fn arbitrary_small_inputs_never_panic() {
    let mut state = 0x1234_5678u32;
    for length in 0..512 {
        let bytes: Vec<u8> = (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        assert!(OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()).is_err());
    }
}

#[test]
fn incomplete_and_trailing_deflate_streams_are_rejected() {
    // Keep sizes/offsets internally consistent, isolating stream-end validation.
    let original = zip(
        &[("a.dat".into(), b"hello hello hello".to_vec())],
        true,
        false,
    );
    let cd = central(&original);
    let compressed = u32::from_le_bytes(original[18..22].try_into().unwrap());
    let data_end = 30 + "a.dat".len() + compressed as usize;
    for trailing in [false, true] {
        let mut bytes = original.clone();
        let (new_cd, new_compressed) = if trailing {
            bytes.insert(data_end, 0);
            (cd + 1, compressed + 1)
        } else {
            bytes.remove(data_end - 1);
            (cd - 1, compressed - 1)
        };
        u32w(&mut bytes, 18, new_compressed);
        u32w(&mut bytes, new_cd + 20, new_compressed);
        let end = bytes.len() - 22;
        u32w(&mut bytes, end + 16, new_cd as u32);
        assert!(
            matches!(
                OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
                Err(PackageError::InvalidZip(_))
            ),
            "trailing={trailing}"
        );
    }
}

#[test]
fn descriptor_without_signature_is_supported_and_bad_descriptor_rejected() {
    let mut bytes = zip(&minimal(1, false), true, true);
    let first_descriptor = bytes.windows(4).position(|b| b == b"PK\x07\x08").unwrap();
    bytes[first_descriptor + 4] ^= 1;
    assert!(matches!(
        OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()),
        Err(PackageError::InvalidZip(_))
    ));
    let mut bytes = zip(&[("a.dat".into(), b"hello".to_vec())], true, true);
    let descriptor = bytes.windows(4).position(|b| b == b"PK\x07\x08").unwrap();
    bytes.drain(descriptor..descriptor + 4);
    let cd = central(&bytes);
    let end = bytes.len() - 22;
    u32w(&mut bytes, end + 16, cd as u32);
    // This is a valid ZIP but deliberately lacks OPC metadata.
    assert_eq!(
        OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()).unwrap_err(),
        PackageError::MissingPart("[Content_Types].xml".into())
    );
}

#[test]
fn input_mutations_never_panic() {
    let original = zip(&minimal(1, true), true, true);
    for offset in 0..original.len() {
        let mut bytes = original.clone();
        bytes[offset] ^= 0xff;
        let _ = OoxmlPackage::from_bytes(&bytes, SecurityLimits::default());
    }
}

#[test]
fn zip_comments_and_zero_slides_are_supported() {
    let mut bytes = zip(&minimal(0, false), true, false);
    let end = bytes.len() - 22;
    bytes[end + 20..end + 22].copy_from_slice(&7u16.to_le_bytes());
    bytes.extend_from_slice(b"comment");
    let package = OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()).unwrap();
    assert_eq!(package.inspect_pptx().unwrap().slide_count(), 0);
}

#[test]
fn total_uncompressed_limit_uses_the_whole_archive() {
    let parts = minimal(1, false);
    let total = parts
        .iter()
        .map(|(_, bytes)| bytes.len() as u64)
        .sum::<u64>();
    let bytes = zip(&parts, true, false);
    OoxmlPackage::from_bytes(
        &bytes,
        SecurityLimits {
            max_total_uncompressed_size: total,
            ..SecurityLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        OoxmlPackage::from_bytes(
            &bytes,
            SecurityLimits {
                max_total_uncompressed_size: total - 1,
                ..SecurityLimits::default()
            }
        )
        .unwrap_err(),
        PackageError::LimitExceeded {
            limit: "max_total_uncompressed_size"
        }
    );
}

#[test]
fn relationship_metadata_and_thumbnails_are_not_presentation_media() {
    let mut parts = minimal(1, true);
    parts.push(("ppt/media/_rels/image1.png.rels".into(), rels("")));
    parts.push(("docProps/thumbnail.png".into(), vec![]));
    assert_eq!(
        open(&parts).unwrap().inspect_pptx().unwrap().media,
        ["ppt/media/image1.png"]
    );
}

#[test]
fn macro_enabled_packages_are_not_classified_as_pptx() {
    let mut parts = minimal(1, false);
    let types = parts
        .iter_mut()
        .find(|(name, _)| name == "[Content_Types].xml")
        .unwrap();
    types.1 = String::from_utf8(types.1.clone())
        .unwrap()
        .replace(
            support::MAIN_TYPE,
            "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
        )
        .into_bytes();
    assert!(!open(&parts).unwrap().is_pptx());
}

#[test]
fn relationship_cycles_do_not_trigger_recursive_loading() {
    let mut parts = minimal(1, false);
    parts.push((
        "ppt/slides/_rels/slide1.xml.rels".into(),
        rels(&relation("cycle", "custom", "../presentation.xml")),
    ));
    assert_eq!(
        open(&parts).unwrap().inspect_pptx().unwrap().slide_count(),
        1
    );
}

#[test]
fn valid_parts_decompress_across_multiple_output_chunks() {
    let mut parts = minimal(1, false);
    let payload: Vec<u8> = (0..100_000).map(|i| (i % 251) as u8).collect();
    parts.push(("large.dat".into(), payload.clone()));
    let package = open(&parts).unwrap();
    assert_eq!(package.part("large.dat").unwrap().bytes(), payload);
}

#[test]
fn part_names_and_resolved_relationship_paths_have_configurable_bounds() {
    let bytes = zip(&minimal(1, false), true, false);
    assert_eq!(
        OoxmlPackage::from_bytes(
            &bytes,
            SecurityLimits {
                max_path_length: 10,
                ..SecurityLimits::default()
            }
        )
        .unwrap_err(),
        PackageError::LimitExceeded {
            limit: "max_path_length"
        }
    );
    let mut parts = minimal(1, false);
    parts.push((format!("{}.dat", "a".repeat(1_024)), vec![]));
    assert_eq!(
        open(&parts).unwrap_err(),
        PackageError::LimitExceeded {
            limit: "max_path_length"
        }
    );
}
