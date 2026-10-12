//! cargo run --offline -p o2p-pptx --example generate_fixtures
#[path = "../tests/support/mod.rs"]
mod support;

use std::{fs, path::Path};

fn main() -> std::io::Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pptx");
    fs::create_dir_all(&directory)?;
    for (name, parts) in support::fixtures()
        .into_iter()
        .chain(support::content_fixtures())
        .chain(support::content_error_fixtures())
        .chain(support::shape_fixtures())
        .chain(support::image_fixtures())
        .chain(support::text_fixtures())
        .chain(support::font_fixtures())
        .chain(support::wrapping_fixtures())
        .chain(support::transform_fixtures())
    {
        fs::write(directory.join(name), support::zip(&parts, true, false))?;
    }
    Ok(())
}
