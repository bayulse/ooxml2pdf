mod support;

use std::path::Path;

use o2p_core::Size;
use o2p_ooxml::{OoxmlPackage, SecurityLimits};
use o2p_pptx::{ParseLimits, PptxError, PptxWarning, Presentation};
use support::{STANDARD_SIZE, WIDE_SIZE, deck, ids, presentation, relation, rels, replace, zip};

fn parse(parts: &[(String, Vec<u8>)]) -> Result<Presentation, PptxError> {
    parse_limited(parts, ParseLimits::default())
}

fn parse_limited(
    parts: &[(String, Vec<u8>)],
    limits: ParseLimits,
) -> Result<Presentation, PptxError> {
    let package = OoxmlPackage::from_bytes(&zip(parts, false, false), SecurityLimits::default())?;
    Presentation::parse(&package, limits)
}

#[test]
fn standard_size_and_normalized_model_outlive_the_package() {
    let model = parse(&deck(1, STANDARD_SIZE)).unwrap();
    assert_eq!(model.size().width_emu(), 9_144_000);
    assert_eq!(model.size().height_emu(), 6_858_000);
    assert_eq!(
        model.size().to_points(),
        Size {
            width: 720.0,
            height: 540.0
        }
    );
    assert_eq!(model.slides()[0].id(), 256);
    assert_eq!(model.slides()[0].part_name(), "ppt/slides/slide1.xml");
    assert_eq!(
        model.warnings(),
        &[PptxWarning::SlideContentNotRendered {
            slide_id: 256,
            part: "ppt/slides/slide1.xml".into(),
        }]
    );
}

#[test]
fn slide_order_uses_id_list_then_relationship_targets_not_filenames_or_ids() {
    let model = parse(&support::reordered()).unwrap();
    assert_eq!(
        model
            .slides()
            .iter()
            .map(|slide| slide.part_name())
            .collect::<Vec<_>>(),
        [
            "ppt/slides/slide3.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml"
        ]
    );
    assert_eq!(
        model
            .slides()
            .iter()
            .map(|slide| slide.id())
            .collect::<Vec<_>>(),
        [900, 300, 600]
    );
    let display = model.to_blank_display_list();
    assert_eq!(display.pages.len(), model.slides().len());
    assert!(
        display
            .pages
            .iter()
            .all(|page| page.ops.is_empty() && page.size == model.size().to_points())
    );
}

#[test]
fn orphan_slides_are_inventory_only_and_slide_content_is_never_parsed() {
    let mut parts = deck(3, WIDE_SIZE);
    replace(
        &mut parts,
        "ppt/presentation.xml",
        presentation(&format!("{}{WIDE_SIZE}", ids(&[2, 1]))),
    );
    // Even malformed slide content must remain opaque at this milestone.
    replace(
        &mut parts,
        "ppt/slides/slide2.xml",
        b"not even XML; <chart><SmartArt>".to_vec(),
    );
    let model = parse(&parts).unwrap();
    assert_eq!(model.slides().len(), 2);
    assert_eq!(model.slides()[0].part_name(), "ppt/slides/slide2.xml");
    assert_eq!(model.to_blank_display_list().pages.len(), 2);
    assert_eq!(model.warnings().len(), 2);
}

#[test]
fn follows_main_part_location_and_relative_relationships() {
    let mut parts = deck(1, STANDARD_SIZE);
    for (name, bytes) in &mut parts {
        match name.as_str() {
            "ppt/presentation.xml" => *name = "custom/deck.xml".into(),
            "ppt/_rels/presentation.xml.rels" => {
                *name = "custom/_rels/deck.xml.rels".into();
                *bytes = rels(&relation("rId1", "slide", "../ppt/slides/./slide1.xml"));
            }
            "[Content_Types].xml" => {
                *bytes = String::from_utf8(bytes.clone())
                    .unwrap()
                    .replace("/ppt/presentation.xml", "/custom/deck.xml")
                    .into_bytes()
            }
            "_rels/.rels" => {
                *bytes = rels(&relation("rOffice", "officeDocument", "custom/deck.xml"))
            }
            _ => {}
        }
    }
    assert_eq!(
        parse(&parts).unwrap().slides()[0].part_name(),
        "ppt/slides/slide1.xml"
    );
}

#[test]
fn namespace_prefixes_entities_comments_and_utf16_do_not_change_semantics() {
    let mut parts = deck(1, STANDARD_SIZE);
    let metadata = format!(
        "<?xml version='1.0' encoding='UTF-8'?><!--synthetic--><presentation xmlns='{}' xmlns:link='{}'><sldIdLst><sldId id='256' link:id='rId&#49;'></sldId></sldIdLst><sldSz cx='  +9144000  ' cy='6858000'/></presentation>",
        support::PML,
        support::OFFICE_RELS
    );
    replace(
        &mut parts,
        "ppt/presentation.xml",
        metadata.as_bytes().to_vec(),
    );
    let expected = parse(&parts).unwrap();
    for little in [true, false] {
        let text = metadata.replace("UTF-8", "UTF-16");
        let mut bytes = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        bytes.extend(text.encode_utf16().flat_map(|word| {
            if little {
                word.to_le_bytes()
            } else {
                word.to_be_bytes()
            }
        }));
        replace(&mut parts, "ppt/presentation.xml", bytes);
        assert_eq!(parse(&parts).unwrap(), expected);
    }
    let text = format!("\u{feff}{}", metadata);
    replace(&mut parts, "ppt/presentation.xml", text.into_bytes());
    assert_eq!(parse(&parts).unwrap(), expected);
}

#[test]
fn strict_namespaces_and_relationship_types_work() {
    let mut parts = deck(1, STANDARD_SIZE);
    for (name, bytes) in &mut parts {
        if name.ends_with("presentation.xml") || name.ends_with(".rels") {
            *bytes = String::from_utf8(bytes.clone())
                .unwrap()
                .replace(
                    support::PML,
                    "http://purl.oclc.org/ooxml/presentationml/main",
                )
                .replace(
                    support::OFFICE_RELS,
                    "http://purl.oclc.org/ooxml/officeDocument/relationships",
                )
                .into_bytes();
        }
    }
    assert_eq!(parse(&parts).unwrap().slides().len(), 1);
}

#[test]
fn opaque_presentation_metadata_returns_structured_warnings() {
    let mut parts = deck(1, STANDARD_SIZE);
    replace(
        &mut parts,
        "ppt/presentation.xml",
        presentation(&format!(
            "<p:sldMasterIdLst><p:sldMasterId id='2147483648' r:id='opaque'/></p:sldMasterIdLst>{}{}<p:extLst><p:ext><unknown xmlns='urn:extension'/></p:ext></p:extLst>",
            ids(&[1]),
            STANDARD_SIZE
        )),
    );
    let model = parse(&parts).unwrap();
    assert_eq!(model.warnings().len(), 3);
    assert!(
        matches!(&model.warnings()[0], PptxWarning::UnsupportedPresentationMetadata { part, name } if part == "ppt/presentation.xml" && name.ends_with("sldMasterIdLst"))
    );
    assert!(model.to_blank_display_list().pages[0].ops.is_empty());
    let xml = format!(
        "<p:presentation xmlns:p='{}' xmlns:r='{}' future='value'><p:sldIdLst><p:sldId id='256' r:id='rId1' future='value'/></p:sldIdLst><p:sldSz cx='9144000' cy='6858000' future='value'/></p:presentation>",
        support::PML,
        support::OFFICE_RELS
    );
    replace(&mut parts, "ppt/presentation.xml", xml.into_bytes());
    assert_eq!(parse(&parts).unwrap().warnings().len(), 4);
}

#[test]
fn missing_or_invalid_slide_sizes_return_metadata_errors() {
    for size in [
        "",
        "<p:sldSz/>",
        "<p:sldSz cx='9144000'/>",
        "<p:sldSz cx='0' cy='6858000'/>",
        "<p:sldSz cx='-9144000' cy='6858000'/>",
        "<p:sldSz cx='NaN' cy='6858000'/>",
        "<p:sldSz cx='9144000.5' cy='6858000'/>",
        "<p:sldSz cx='4294967296' cy='6858000'/>",
        "<p:sldSz cx='51206401' cy='6858000'/>",
        "<p:sldSz cx='914399' cy='6858000'/>",
        "<p:sldSz cx='9144000' cy='0'/>",
        "<p:sldSz cx='9144000' cy='6858000'><p:child/></p:sldSz>",
        "<p:sldSz xmlns:p='urn:wrong' cx='9144000' cy='6858000'/>",
    ] {
        assert!(
            matches!(
                parse(&deck(1, size)),
                Err(PptxError::InvalidMetadata { .. })
            ),
            "{size}"
        );
    }
    for (width, height) in [(914_400, 51_206_400), (51_206_400, 914_400)] {
        assert!(parse(&deck(1, &format!("<p:sldSz cx='{width}' cy='{height}'/>"))).is_ok());
    }
}

#[test]
fn rejects_ambiguous_or_malformed_presentation_metadata() {
    for inner in [
        format!("{}{STANDARD_SIZE}{STANDARD_SIZE}", ids(&[1])),
        format!("{}{}{STANDARD_SIZE}", ids(&[1]), ids(&[1])),
        STANDARD_SIZE.to_owned(),
        format!("<p:sldIdLst/>{STANDARD_SIZE}"),
        format!("<p:sldIdLst><p:sldId id='256'/></p:sldIdLst>{STANDARD_SIZE}"),
        format!("<p:sldIdLst><p:sldId id='255' r:id='rId1'/></p:sldIdLst>{STANDARD_SIZE}"),
        format!("<p:sldIdLst><p:sldId id='2147483648' r:id='rId1'/></p:sldIdLst>{STANDARD_SIZE}"),
        format!(
            "<p:sldIdLst><p:sldId id='256' r:id='rId1'/><p:sldId id='256' r:id='rId2'/></p:sldIdLst>{STANDARD_SIZE}"
        ),
        format!(
            "<p:sldIdLst><p:sldId id='256' r:id='rId1'/><p:sldId id='257' r:id='rId1'/></p:sldIdLst>{STANDARD_SIZE}"
        ),
        format!("<p:sldIdLst><p:unknown/></p:sldIdLst>{STANDARD_SIZE}"),
        format!("<p:sldIdLst>text</p:sldIdLst>{STANDARD_SIZE}"),
        format!(
            "<p:sldIdLst><p:sldId id='256' r:id='rId1'><p:child/></p:sldId></p:sldIdLst>{STANDARD_SIZE}"
        ),
        format!("{}text{STANDARD_SIZE}", ids(&[1])),
    ] {
        let mut parts = deck(2, STANDARD_SIZE);
        replace(&mut parts, "ppt/presentation.xml", presentation(&inner));
        assert!(
            matches!(parse(&parts), Err(PptxError::InvalidMetadata { .. })),
            "{inner}"
        );
    }
}

#[test]
fn invalid_or_external_slide_relationships_fail_without_fetching() {
    for relationships in [
        rels(""),
        rels(&relation("rId1", "image", "slides/slide1.xml")),
        rels(&format!(
            "<Relationship Id='rId1' Type='{}/slide' Target='https://example.invalid/slide.xml' TargetMode='External'/>",
            support::OFFICE_RELS
        )),
        rels(&relation("rId1", "slide", "presentation.xml")),
    ] {
        let mut parts = deck(1, STANDARD_SIZE);
        replace(&mut parts, "ppt/_rels/presentation.xml.rels", relationships);
        assert!(
            matches!(parse(&parts), Err(PptxError::InvalidSlideRelationship { relationship_id, .. }) if relationship_id == "rId1")
        );
    }
    let mut parts = deck(1, STANDARD_SIZE);
    replace(
        &mut parts,
        "ppt/_rels/presentation.xml.rels",
        rels(&relation("rId1", "slide", "slides/missing.xml")),
    );
    assert!(matches!(
        parse(&parts),
        Err(PptxError::Package(o2p_ooxml::PackageError::MissingPart(_)))
    ));
}

#[test]
fn rejects_hostile_or_malformed_xml_and_unsupported_encodings() {
    let valid = presentation(&format!("{}{STANDARD_SIZE}", ids(&[1])));
    let text = String::from_utf8(valid.clone()).unwrap();
    let mut cases = vec![
        vec![],
        b"<p:presentation".to_vec(),
        b"<wrong/>".to_vec(),
        format!("<!DOCTYPE p:presentation>{text}").into_bytes(),
        format!("<!DOCTYPE p:presentation [<!ENTITY x SYSTEM 'file:///etc/passwd'>]>{text}")
            .into_bytes(),
        format!("<?xml version='1.1'?>{text}").into_bytes(),
        format!("<?xml version='1&#46;0'?>{text}").into_bytes(),
        format!(
            "<?xml version='1.0'><x>{}{}</x>?>{text}",
            "<x>".repeat(1000),
            "</x>".repeat(1000)
        )
        .into_bytes(),
        format!("<?xml version='1.0' encoding='ISO-8859-1'?>{text}").into_bytes(),
        format!("<?xml version='1.0' encoding='UTF-16'?>{text}").into_bytes(),
        format!("{text}{text}").into_bytes(),
        text.replace("cx='9144000'", "cx='&unknown;'").into_bytes(),
        text.replace("cx='9144000'", "cx='9144000' cx='9144000'")
            .into_bytes(),
        text.replace("</p:presentation>", "</p:wrong>").into_bytes(),
        vec![0xff, 0xfe, 0],
        vec![0xff, 0xfe, 0, 0xd8],
        vec![0xff],
    ];
    cases.extend((0..valid.len()).map(|len| valid[..len].to_vec()));
    for bytes in cases {
        let mut parts = deck(1, STANDARD_SIZE);
        replace(&mut parts, "ppt/presentation.xml", bytes);
        assert!(parse(&parts).is_err());
    }
}

#[test]
fn presentation_resource_limits_are_enforced() {
    let parts = deck(3, STANDARD_SIZE);
    for (limits, name) in [
        (
            ParseLimits {
                max_xml_size: 1,
                ..ParseLimits::default()
            },
            "max_xml_size",
        ),
        (
            ParseLimits {
                max_xml_nodes: 2,
                ..ParseLimits::default()
            },
            "max_xml_nodes",
        ),
        (
            ParseLimits {
                max_xml_depth: 2,
                ..ParseLimits::default()
            },
            "max_xml_depth",
        ),
        (
            ParseLimits {
                max_slides: 2,
                ..ParseLimits::default()
            },
            "max_slides",
        ),
    ] {
        assert_eq!(
            parse_limited(&parts, limits).unwrap_err(),
            PptxError::LimitExceeded { limit: name }
        );
    }
    assert!(
        parse_limited(
            &parts,
            ParseLimits {
                max_slides: 3,
                ..ParseLimits::default()
            }
        )
        .is_ok()
    );
    let mut nested = deck(1, STANDARD_SIZE);
    replace(
        &mut nested,
        "ppt/presentation.xml",
        presentation(&format!(
            "{}{STANDARD_SIZE}<p:extLst>{}{}</p:extLst>",
            ids(&[1]),
            "<p:ext>".repeat(129),
            "</p:ext>".repeat(129)
        )),
    );
    assert_eq!(
        parse(&nested).unwrap_err(),
        PptxError::LimitExceeded {
            limit: "max_xml_depth"
        }
    );
    assert_eq!(
        parse_limited(
            &nested,
            ParseLimits {
                max_xml_depth: usize::MAX,
                ..ParseLimits::default()
            }
        )
        .unwrap_err(),
        PptxError::LimitExceeded {
            limit: "max_xml_depth"
        }
    );
    replace(
        &mut nested,
        "ppt/presentation.xml",
        presentation(&format!(
            "{}{STANDARD_SIZE}<p:extLst>{}{}</p:extLst>",
            ids(&[1]),
            "<p:ext>".repeat(62),
            "</p:ext>".repeat(62)
        )),
    );
    assert!(parse(&nested).is_ok());
}

#[test]
fn resource_preflight_handles_opaque_tokens_and_bounds_warning_names() {
    let mut parts = deck(1, STANDARD_SIZE);
    let text = format!(
        "<!-- <!DOCTYPE fake> {} --><?test {}?><p:extLst xmlns:future='{}'><future:opaque attr='&lt;x&gt;'><![CDATA[{}]]></future:opaque></p:extLst>",
        "<x>".repeat(1000),
        "<x>".repeat(1000),
        "x".repeat(200_000),
        "<x>".repeat(1000)
    );
    replace(
        &mut parts,
        "ppt/presentation.xml",
        presentation(&format!("{}{STANDARD_SIZE}{text}", ids(&[1]))),
    );
    assert!(parse(&parts).is_ok());
    let text = format!("<future:opaque xmlns:future='{}'/>", "x".repeat(200_000));
    replace(
        &mut parts,
        "ppt/presentation.xml",
        presentation(&format!("{}{STANDARD_SIZE}{text}", ids(&[1]))),
    );
    let model = parse(&parts).unwrap();
    assert!(
        matches!(&model.warnings()[0], PptxWarning::UnsupportedPresentationMetadata { name, .. } if name.len() < 200 && name.contains('…'))
    );
}

#[test]
fn committed_fixtures_are_reproducible_and_generate_blank_pdfs_with_real_dimensions() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pptx");
    for (name, parts) in support::fixtures() {
        let bytes = std::fs::read(directory.join(name)).unwrap();
        assert_eq!(bytes, zip(&parts, true, false), "{name}");
        let package = OoxmlPackage::from_bytes(&bytes, SecurityLimits::default()).unwrap();
        let model = Presentation::parse(&package, ParseLimits::default());
        if name == "deep-presentation.pptx" {
            assert_eq!(
                model.unwrap_err(),
                PptxError::LimitExceeded {
                    limit: "max_xml_depth"
                }
            );
            continue;
        }
        if name.contains("slide-size") {
            assert!(matches!(model, Err(PptxError::InvalidMetadata { .. })));
            continue;
        }
        let model = model.unwrap();
        let display = model.to_blank_display_list();
        let pdf_bytes = o2p_pdf::render(&display).unwrap();
        assert_eq!(pdf_bytes, o2p_pdf::render(&display).unwrap());
        let pdf = lopdf::Document::load_mem(&pdf_bytes).unwrap();
        let pages = pdf.get_pages();
        let count = if name.starts_with("one") { 1 } else { 3 };
        let width = if name.contains("16x9") { 960.0 } else { 720.0 };
        assert_eq!(model.slides().len(), count);
        assert_eq!(pages.len(), count);
        for (index, page_id) in pages {
            let media = pdf
                .get_dictionary(page_id)
                .unwrap()
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap();
            let numbers: Vec<_> = media
                .iter()
                .map(|value| value.as_float().unwrap())
                .collect();
            assert_eq!(numbers, [0.0, 0.0, width, 540.0], "{name}, page {index}");
            assert!(pdf.get_page_content(page_id).unwrap().is_empty());
        }
    }
}
