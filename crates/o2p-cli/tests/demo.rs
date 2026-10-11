use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "o2p-cli-test-{}-{}",
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

#[test]
fn demo_command_writes_a_deterministic_readable_pdf() {
    let directory = TestDirectory::new();
    let first = directory.0.join("demo one.pdf");
    let second = directory.0.join("demo two.pdf");
    for path in [&first, &second] {
        let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
            .arg("demo")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let bytes = fs::read(first).unwrap();
    assert_eq!(bytes, fs::read(second).unwrap());
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("ooxml2pdf - Milestone 0"));
    assert!(text.contains("Common DisplayList -> PDF"));
}

#[test]
fn missing_arguments_fail_with_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("demo")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}

#[test]
fn unwritable_destination_returns_an_error() {
    let directory = TestDirectory::new();
    let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("demo")
        .arg(&directory.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot write"));
}

#[test]
fn help_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_o2p"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("o2p demo <output.pdf>"));
}
