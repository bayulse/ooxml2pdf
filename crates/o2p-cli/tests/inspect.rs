use std::{path::Path, process::Command};
fn inspect(name: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("inspect")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/pptx")
                .join(name),
        )
        .output()
        .unwrap()
}
#[test]
fn inspection_lists_synthetic_slide_and_media_parts() {
    let output = inspect("one-image.pptx");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Format: PPTX\nSlides: 1\nMedia: 1\n\nSlides:\n- ppt/slides/slide1.xml\n\nMedia:\n- ppt/media/image1.png\n"
    );
    let output = inspect("three-slides.pptx");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Format: PPTX\nSlides: 3\nMedia: 0\n\nSlides:\n- ppt/slides/slide1.xml\n- ppt/slides/slide2.xml\n- ppt/slides/slide3.xml\n\nMedia:\n"
    );
}
#[test]
fn inspection_reports_file_and_package_errors_without_partial_output() {
    for name in [
        "missing.pptx",
        "malformed-relationships.pptx",
        "malformed-xml.pptx",
        "path-traversal.pptx",
        "compression-abuse.pptx",
    ] {
        let output = inspect(name);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot inspect"));
    }
}
#[test]
fn inspect_requires_exactly_one_nonempty_path_and_help_describes_it() {
    for args in [
        vec!["inspect"],
        vec!["inspect", ""],
        vec!["inspect", "file.pptx", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("o2p inspect <file.pptx>"));
}
