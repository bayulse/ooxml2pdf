use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "o2p-convert-test-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/pptx")
        .join(name)
}

fn convert(input: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("convert")
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap()
}

fn assert_only_missing_layout_warning(stderr: &[u8]) {
    let warnings = String::from_utf8_lossy(stderr);
    assert_eq!(warnings.lines().count(), 1, "{warnings}");
    assert!(
        warnings.contains("missing slideLayout relationship"),
        "{warnings}"
    );
}

#[test]
fn inheritance_cli_embeds_text_preserves_layers_and_fails_before_output_write() {
    let directory = TestDirectory::new();
    let destination = directory.0.join("inheritance.pdf");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_o2p"))
            .arg("convert")
            .arg(fixture("layout-master-inheritance.pptx"))
            .arg("--font-dir")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
            .arg("-o")
            .arg(&destination)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let warnings = String::from_utf8_lossy(&output.stderr);
    assert!(!warnings.contains("missing slideLayout") && !warnings.contains("missing slideMaster"));
    assert!(!warnings.contains("unresolved") && !warnings.contains("MissingFont"));
    let bytes = fs::read(&destination).unwrap();
    assert!(run().status.success());
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert!(
        extracted.contains("Layout and master inheritance") && extracted.contains("Local override")
    );
    assert!(!extracted.contains("sample"));
    assert!(bytes.windows(10).any(|w| w == b"/FontFile2"));
    assert!(bytes.windows(10).any(|w| w == b"/ToUnicode"));
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap();
    assert!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "re")
            .count()
            >= 3
    );
    assert!(content.operations.iter().any(|op| op.operator == "c"));
    for name in [
        "malformed-layout-relationship.pptx",
        "malformed-master-relationship.pptx",
    ] {
        assert!(!convert(&fixture(name), &destination).status.success());
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        let inspection = Command::new(env!("CARGO_BIN_EXE_o2p"))
            .arg("inspect")
            .arg(fixture(name))
            .output()
            .unwrap();
        assert!(inspection.status.success());
    }
}

#[test]
fn theme_cli_renders_palette_embedded_font_and_preserves_output_on_bad_theme() {
    let directory = TestDirectory::new();
    let destination = directory.0.join("theme-colors-and-fonts.pdf");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_o2p"))
            .arg("convert")
            .arg(fixture("theme-colors-and-fonts.pptx"))
            .arg("--font-dir")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"))
            .arg("-o")
            .arg(&destination)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let warnings = String::from_utf8_lossy(&output.stderr);
    assert!(!warnings.contains("theme color unavailable"));
    assert!(!warnings.contains("Substitution") && !warnings.contains("MissingFont"));
    let bytes = fs::read(&destination).unwrap();
    assert!(run().status.success());
    assert_eq!(bytes, fs::read(&destination).unwrap());
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert!(
        extracted.contains("Résumé")
            && extracted.contains("Major Latin")
            && extracted.contains("Minor Latin")
    );
    assert!(bytes.windows(10).any(|w| w == b"/FontFile2"));
    assert!(bytes.windows(10).any(|w| w == b"/ToUnicode"));
    let output = convert(&fixture("malformed-theme.pptx"), &destination);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid theme"));
    assert_eq!(bytes, fs::read(&destination).unwrap());
    let inspection = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("inspect")
        .arg(fixture("malformed-theme.pptx"))
        .output()
        .unwrap();
    assert!(inspection.status.success());
    let missing = convert(&fixture("missing-theme.pptx"), &destination);
    assert!(missing.status.success());
    let warnings = String::from_utf8_lossy(&missing.stderr);
    assert!(warnings.contains("schemeClr") && warnings.contains("+mj-lt unresolved"));
}

#[test]
fn mixed_transforms_cli_keeps_native_images_vectors_and_searchable_rotated_text() {
    let directory = TestDirectory::new();
    let destination = directory.0.join("mixed-transforms.pdf");
    for configured in [false, true] {
        let run = || {
            let mut command = Command::new(env!("CARGO_BIN_EXE_o2p"));
            command
                .arg("convert")
                .arg(fixture("mixed-transforms.pptx"))
                .arg("-o")
                .arg(&destination);
            if configured {
                command
                    .arg("--font-dir")
                    .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts"));
            }
            command.output().unwrap()
        };
        let output = run();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let warnings = String::from_utf8_lossy(&output.stderr);
        assert!(!warnings.contains("element omitted") && !warnings.contains("picture omitted"));
        let bytes = fs::read(&destination).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|o| o.operator == "cm")
                .count(),
            11
        ); // 9 element scopes + 2 image placements.
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|o| o.operator == "Do")
                .count(),
            2
        );
        let text: String = pdf
            .extract_text(&[1])
            .unwrap()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        for label in [
            "Rectangle90",
            "Rotatedrealtext",
            "ABChorizontal",
            "ABCvertical",
        ] {
            assert!(text.contains(label), "{label}: {text}");
        }
        if configured {
            assert!(
                pdf.objects
                    .values()
                    .any(|o| o.as_dict().is_ok_and(|d| d.has(b"ToUnicode")))
            );
            assert!(
                String::from_utf8_lossy(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                    .contains("/ActualText")
            );
        }
        assert!(run().status.success());
        assert_eq!(bytes, fs::read(&destination).unwrap());
    }
}

#[test]
fn wrapped_cli_fixture_preserves_fonts_styles_positions_and_searchable_text() {
    let directory = TestDirectory::new();
    let destination = directory.0.join("mixed-runs-wrapped.pdf");
    let font_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    let convert = || {
        Command::new(env!("CARGO_BIN_EXE_o2p"))
            .arg("convert")
            .arg(fixture("mixed-runs-wrapped.pptx"))
            .arg("-o")
            .arg(&destination)
            .arg("--font-dir")
            .arg(&font_directory)
            .arg("--fallback-family")
            .arg("O2P Test Sans")
            .output()
            .unwrap()
    };
    let output = convert();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let warnings = String::from_utf8_lossy(&output.stderr);
    assert!(!warnings.contains("MissingGlyph") && !warnings.contains(": Overflow"));
    let bytes = fs::read(&destination).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let text: String = pdf
        .extract_text(&[1])
        .unwrap()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(text.contains("Normaltextleadsinto"));
    assert!(text.contains("bolditalicredwordscontinueacrossseveralwrappedlines"));
    assert!(text.contains("andnormaltextresumeshere."));
    assert!(
        pdf.objects
            .values()
            .any(|o| o.as_dict().is_ok_and(|d| d.has(b"FontFile2")))
    );
    assert!(
        pdf.objects
            .values()
            .filter(|o| o.as_dict().is_ok_and(|d| d.has(b"ToUnicode")))
            .count()
            >= 2
    );
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap();
    let baselines: std::collections::BTreeSet<_> = content
        .operations
        .iter()
        .filter(|op| op.operator == "Tm")
        .map(|op| op.operands[5].as_float().unwrap().to_bits())
        .collect();
    assert!(baselines.len() >= 4);
    assert!(content.operations.iter().any(|op| op.operator == "BDC"));
    assert!(content.operations.iter().any(|op| {
        op.operator == "rg"
            && op.operands[0]
                .as_float()
                .is_ok_and(|red| (red - 200.0 / 255.0).abs() < 0.00001)
    }));
    assert!(convert().status.success());
    assert_eq!(fs::read(destination).unwrap(), bytes);
}

#[test]
fn configured_fonts_embed_unicode_without_missing_japanese_and_preserve_cli_output_on_error() {
    let directory = TestDirectory::new();
    let output = directory.0.join("embedded.pdf");
    let font_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    let result = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("convert")
        .arg(fixture("unicode-text.pptx"))
        .arg("-o")
        .arg(&output)
        .arg("--font-dir")
        .arg(&font_directory)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let warnings = String::from_utf8(result.stderr).unwrap();
    assert!(warnings.contains("Substitution"));
    assert!(!warnings.contains("MissingGlyph") && !warnings.contains("rendered as '?'"));
    let first = fs::read(&output).unwrap();
    let pdf = lopdf::Document::load_mem(&first).unwrap();
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Résumé") && text.contains("日本語"));
    let result = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("convert")
        .arg(fixture("unicode-text.pptx"))
        .arg("-o")
        .arg(&output)
        .arg("--font-dir")
        .arg(&font_directory)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(first, fs::read(&output).unwrap());
    let broken = directory.0.join("bad.ttf");
    fs::write(&broken, b"invalid font").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("convert")
        .arg(fixture("unicode-text.pptx"))
        .arg("-o")
        .arg(&output)
        .arg("--font")
        .arg(&broken)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(first, fs::read(&output).unwrap());
    assert!(String::from_utf8_lossy(&result.stderr).contains("InvalidFont"));
}

#[test]
fn individual_font_paths_and_configured_fallback_families_are_accepted() {
    let directory = TestDirectory::new();
    let output = directory.0.join("explicit.pdf");
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
    let result = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("convert")
        .arg(fixture("unicode-text.pptx"))
        .arg("-o")
        .arg(&output)
        .arg("--font")
        .arg(fonts.join("O2PTestSans-Regular.ttf"))
        .arg("--font")
        .arg(fonts.join("O2PTestJapanese-Regular.ttf"))
        .args([
            "--fallback-family",
            "O2P Test Sans",
            "--fallback-family",
            "O2P Test Japanese",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        lopdf::Document::load_mem(&fs::read(output).unwrap())
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains("日本語")
    );
}

#[test]
fn conversion_preserves_deterministic_blank_pages_with_correct_dimensions() {
    let directory = TestDirectory::new();
    for (name, count, width) in [
        ("one-slide-standard.pptx", 1, 720.0),
        ("three-slides-standard.pptx", 3, 720.0),
        ("three-slides-16x9.pptx", 3, 960.0),
        ("reordered-slides.pptx", 3, 720.0),
    ] {
        let destination = directory.0.join("output with spaces.pdf");
        let output = convert(&fixture(name), &destination);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("{count} pages")));
        let warnings = String::from_utf8(output.stderr).unwrap();
        {
            assert_eq!(warnings.matches("clrMapOvr").count(), count);
            if name == "reordered-slides.pptx" {
                let order: Vec<_> = warnings
                    .lines()
                    .filter(|line| line.contains("clrMapOvr"))
                    .collect();
                assert!(order[0].contains("slide3.xml"));
                assert!(order[1].contains("slide1.xml"));
                assert!(order[2].contains("slide2.xml"));
            }
        }
        let bytes = fs::read(&destination).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let pages = pdf.get_pages();
        assert_eq!(pages.len(), count);
        for page_id in pages.values() {
            let media_box = pdf
                .get_dictionary(*page_id)
                .unwrap()
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap();
            let numbers: Vec<_> = media_box
                .iter()
                .map(|value| value.as_float().unwrap())
                .collect();
            assert_eq!(numbers, [0.0, 0.0, width, 540.0]);
            assert!(pdf.get_page_content(*page_id).unwrap().is_empty());
        }
        // Existing destinations are overwritten only after successful parsing.
        fs::write(&destination, b"old output").unwrap();
        assert!(convert(&fixture(name), &destination).status.success());
        assert_eq!(fs::read(&destination).unwrap(), bytes);
    }
}

#[test]
fn conversion_failures_do_not_create_or_overwrite_output() {
    let directory = TestDirectory::new();
    for name in [
        "missing.pptx",
        "missing-slide-size.pptx",
        "malformed-slide-size.pptx",
        "malformed-relationships.pptx",
        "path-traversal.pptx",
        "invalid-slide-root.pptx",
        "deep-slide.pptx",
    ] {
        for existing in [false, true] {
            let destination = directory.0.join(format!("{name}-{existing}.pdf"));
            if existing {
                fs::write(&destination, b"preserve existing output").unwrap();
            }
            let output = convert(&fixture(name), &destination);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot convert"));
            if existing {
                assert_eq!(fs::read(&destination).unwrap(), b"preserve existing output");
            } else {
                assert!(!destination.exists());
            }
        }
    }
}

#[test]
fn filled_rectangle_conversion_contains_visible_fill_with_correct_geometry() {
    let directory = TestDirectory::new();
    let destination = directory.0.join("filled-rectangle.pdf");
    let output = convert(&fixture("filled-rectangle.pptx"), &destination);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_only_missing_layout_warning(&output.stderr);
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 pages"));
    let bytes = fs::read(&destination).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let page = pdf.get_pages()[&1];
    let content = lopdf::content::Content::decode(&pdf.get_page_content(page).unwrap()).unwrap();
    let rect = content
        .operations
        .iter()
        .find(|op| op.operator == "re")
        .unwrap();
    let actual: Vec<_> = rect
        .operands
        .iter()
        .map(|value| value.as_float().unwrap())
        .collect();
    let x = (1_234_567_f64 / 12_700.0) as f32;
    let y = (765_432_f64 / 12_700.0) as f32;
    let w = (3_456_789_f64 / 12_700.0) as f32;
    let h = (2_345_678_f64 / 12_700.0) as f32;
    assert_eq!(actual, [x, 540.0 - y - h, w, h]);
    assert!(content.operations.iter().any(|op| op.operator == "f"));
    assert!(w > 0.0 && h > 0.0);
    let color = &content
        .operations
        .iter()
        .find(|op| op.operator == "rg")
        .unwrap()
        .operands;
    for (actual, channel) in color.iter().zip([32, 96, 208]) {
        assert!((actual.as_float().unwrap() - channel as f32 / 255.0).abs() < 1e-6);
    }
    assert!(
        convert(&fixture("filled-rectangle.pptx"), &destination)
            .status
            .success()
    );
    assert_eq!(bytes, fs::read(&destination).unwrap());
}

#[test]
fn unsupported_slide_style_is_reported_by_cli() {
    let directory = TestDirectory::new();
    let output = convert(
        &fixture("unsupported-style.pptx"),
        &directory.0.join("output.pdf"),
    );
    assert!(output.status.success());
    let warnings = String::from_utf8(output.stderr).unwrap();
    assert!(warnings.contains("Slide 256 (ppt/slides/slide1.xml)"));
    assert!(warnings.contains("schemeClr"));
    assert!(warnings.contains("prstDash"));
}

#[test]
fn conversion_reports_unwritable_destinations_and_bad_arguments() {
    let directory = TestDirectory::new();
    let output = convert(&fixture("one-slide-standard.pptx"), &directory.0);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot write"));
    for args in [
        vec!["convert"],
        vec!["convert", "input.pptx"],
        vec!["convert", "input.pptx", "-o"],
        vec!["convert", "", "-o", "file.pdf"],
        vec!["convert", "input.pptx", "-o", ""],
        vec!["convert", "input.pptx", "--wrong", "file.pdf"],
        vec!["convert", "input.pptx", "-o", "file.pdf", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("o2p convert <input.pptx> -o <output.pdf>")
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("o2p convert"));
    assert!(help.contains("blank pages"));
}

// macOS filesystems require valid Unicode filenames. Exercise raw OS byte paths
// on Linux; argument preservation itself is tested on all Unix platforms.
#[cfg(target_os = "linux")]
#[test]
fn conversion_accepts_non_utf8_os_paths() {
    use std::os::unix::ffi::OsStringExt;
    let directory = TestDirectory::new();
    let input = directory
        .0
        .join(std::ffi::OsString::from_vec(b"input-\xff.pptx".to_vec()));
    let output = directory
        .0
        .join(std::ffi::OsString::from_vec(b"output-\xfe.pdf".to_vec()));
    fs::copy(fixture("one-slide-standard.pptx"), &input).unwrap();
    assert!(convert(&input, &output).status.success());
    assert_eq!(lopdf::Document::load(&output).unwrap().get_pages().len(), 1);
}

#[test]
fn convert_mixed_shapes_renders_rectangle_ellipse_and_line_in_order() {
    let directory = TestDirectory::new();
    let destination = directory.0.join("mixed-shapes.pdf");
    let output = convert(&fixture("mixed-shapes.pptx"), &destination);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_only_missing_layout_warning(&output.stderr);
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 pages"));
    let bytes = fs::read(&destination).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
            .unwrap();
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "re")
            .count(),
        1
    );
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "c")
            .count(),
        8
    );
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "l")
            .count(),
        1
    );
    let paints: Vec<_> = content
        .operations
        .iter()
        .map(|op| op.operator.as_str())
        .filter(|op| matches!(*op, "f" | "S"))
        .collect();
    assert_eq!(paints, ["f", "f", "S", "S"]);
    let line_start = content
        .operations
        .iter()
        .rev()
        .find(|op| op.operator == "m")
        .unwrap();
    let line_end = content
        .operations
        .iter()
        .find(|op| op.operator == "l")
        .unwrap();
    assert_eq!(
        line_start
            .operands
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect::<Vec<_>>(),
        [72.0, 252.0]
    );
    assert_eq!(
        line_end
            .operands
            .iter()
            .map(|v| v.as_float().unwrap())
            .collect::<Vec<_>>(),
        [576.0, 180.0]
    );
    assert!(
        convert(&fixture("mixed-shapes.pptx"), &destination)
            .status
            .success()
    );
    assert_eq!(fs::read(destination).unwrap(), bytes);
}

#[test]
fn image_with_shapes_cli_conversion_contains_raster_and_vector_in_order() {
    let directory = TestDirectory::new();
    for name in [
        "image-with-shapes.pptx",
        "one-png.pptx",
        "one-jpeg.pptx",
        "one-image.pptx",
    ] {
        let destination = directory.0.join("image-with-shapes.pdf");
        let output = convert(&fixture(name), &destination);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if name != "one-image.pptx" {
            assert_only_missing_layout_warning(&output.stderr);
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("hyperlink"));
        }
        let bytes = fs::read(&destination).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        assert!(content.operations.iter().any(|op| op.operator == "Do"));
        assert!(
            pdf.objects
                .values()
                .filter_map(|o| o.as_stream().ok())
                .any(
                    |s| s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok())
                        == Some(b"Image".as_slice())
                )
        );
        if name == "image-with-shapes.pptx" {
            assert_eq!(
                content
                    .operations
                    .iter()
                    .map(|op| op.operator.as_str())
                    .filter(|op| matches!(*op, "f" | "S" | "Do"))
                    .collect::<Vec<_>>(),
                ["f", "f", "Do", "f", "S"]
            );
        }
        assert!(convert(&fixture(name), &destination).status.success());
        assert_eq!(bytes, fs::read(&destination).unwrap());
    }
}

#[test]
fn text_with_image_conversion_contains_vector_raster_and_readable_unicode_text() {
    use lopdf::{Document, content::Content};
    let directory = TestDirectory::new();
    let destination = directory.0.join("text-with-image.pdf");
    let output = convert(&fixture("text-with-image.pptx"), &destination);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&destination).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let extracted = pdf.extract_text(&[1]).unwrap();
    assert!(extracted.contains("Hello Indonesia — Résumé"));
    assert!(extracted.contains("Text and image"));
    let page = pdf.get_pages()[&1];
    let content = Content::decode(&pdf.get_page_content(page).unwrap()).unwrap();
    let painted: Vec<_> = content
        .operations
        .iter()
        .filter(|o| matches!(o.operator.as_str(), "f" | "S" | "Do" | "Tj"))
        .map(|o| o.operator.as_str())
        .collect();
    assert_eq!(painted, ["f", "f", "S", "Tj", "Do", "Tj"]);
    let (resources, _) = pdf.get_page_resources(page).unwrap();
    let objects = resources
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(objects.len(), 1);
    let image = pdf
        .get_object(objects.iter().next().unwrap().1.as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    assert_eq!(
        image.dict.get(b"Subtype").unwrap().as_name().unwrap(),
        b"Image"
    );
    assert!(
        convert(&fixture("text-with-image.pptx"), &destination)
            .status
            .success()
    );
    assert_eq!(fs::read(destination).unwrap(), bytes);
}

#[test]
fn unicode_fallback_and_text_shape_z_order_are_visible_through_cli() {
    use lopdf::{Document, content::Content};
    let directory = TestDirectory::new();
    let destination = directory.0.join("unicode.pdf");
    let output = convert(&fixture("unicode-text.pptx"), &destination);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("rendered as '?'"));
    let pdf = Document::load(&destination).unwrap();
    let extracted = pdf.extract_text(&[1]).unwrap();
    for expected in ["Hello", "Indonesia", "Résumé", "???"] {
        assert!(extracted.contains(expected));
    }
    assert!(
        convert(&fixture("overlapping-text-shape.pptx"), &destination)
            .status
            .success()
    );
    let pdf = Document::load(destination).unwrap();
    let content = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    let painted: Vec<_> = content
        .operations
        .iter()
        .filter(|o| matches!(o.operator.as_str(), "f" | "Tj"))
        .map(|o| o.operator.as_str())
        .collect();
    assert_eq!(painted, ["Tj", "f", "Tj"]);
}
