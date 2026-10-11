//! Run from any directory: cargo run -p o2p-ooxml --example generate_fixtures --offline
#[path = "../tests/support/mod.rs"]
mod support;
use std::{fs, path::Path};
use support::{minimal, relation, rels, zip};
fn main() -> std::io::Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pptx");
    fs::create_dir_all(&directory)?;
    for (name, parts) in [
        ("minimal.pptx", minimal(1, false)),
        ("one-image.pptx", minimal(1, true)),
        ("three-slides.pptx", minimal(3, false)),
        ("malformed-relationships.pptx", {
            let mut parts = minimal(1, false);
            parts
                .iter_mut()
                .find(|(name, _)| name == "_rels/.rels")
                .unwrap()
                .1 = rels("<Relationship Id='broken' Target='ppt/presentation.xml'/>");
            parts
        }),
        ("malformed-xml.pptx", {
            let mut parts = minimal(1, false);
            parts
                .iter_mut()
                .find(|(name, _)| name == "_rels/.rels")
                .unwrap()
                .1 = b"<Relationships><Relationship".to_vec();
            parts
        }),
        ("path-traversal.pptx", {
            let mut parts = minimal(1, false);
            parts.push(("../escape.dat".into(), b"escape".to_vec()));
            parts
        }),
        ("target-traversal.pptx", {
            let mut parts = minimal(1, false);
            parts
                .iter_mut()
                .find(|(name, _)| name == "_rels/.rels")
                .unwrap()
                .1 = rels(&relation("rOffice", "officeDocument", "../../escape.xml"));
            parts
        }),
        ("entry-limit.pptx", {
            let mut parts = minimal(1, false);
            for i in 0..16 {
                parts.push((format!("extra/{i}.dat"), vec![]));
            }
            parts
        }),
        ("compression-abuse.pptx", {
            let mut parts = minimal(1, false);
            parts.push(("bomb.dat".into(), vec![0; 1024 * 1024]));
            parts
        }),
    ] {
        fs::write(directory.join(name), zip(&parts, true, false))?;
    }
    Ok(())
}
