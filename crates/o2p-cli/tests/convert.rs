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

#[test]
fn conversion_preserves_deterministic_blank_pages_with_correct_dimensions() {
    let directory = TestDirectory::new();
    for (name, count, width) in [
        ("one-slide-standard.pptx", 1, 720.0),
        ("three-slides-standard.pptx", 3, 720.0),
        ("three-slides-16x9.pptx", 3, 960.0),
        ("reordered-slides.pptx", 3, 720.0),
        ("one-image.pptx", 1, 720.0),
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
        if name == "one-image.pptx" {
            assert!(warnings.contains("slide relationship"));
            assert!(warnings.contains("image"));
        } else {
            assert_eq!(warnings.matches("clrMapOvr").count(), count);
            if name == "reordered-slides.pptx" {
                let order: Vec<_> = warnings.lines().collect();
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
    assert!(output.stderr.is_empty());
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
    assert!(output.stderr.is_empty());
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
