//! Development adapter for compat/scripts/compat.py. Never linked into o2p or libraries.
use std::{env, error::Error, fs, path::PathBuf, process::ExitCode, time::Instant};

use o2p_pptx::{PptxWarning, TextLayoutWarningKind};

// Small JSON string encoder keeps the evaluation adapter dependency-free.
fn quoted(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            c if c <= '\u{1f}' => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

fn warning(kind: &str, feature: &str, location: &str, message: &str) -> String {
    format!(
        "{{\"kind\":{},\"feature\":{},\"location\":{},\"message\":{}}}",
        quoted(kind),
        quoted(feature),
        quoted(location),
        quoted(message)
    )
}

fn parser_warning(value: &PptxWarning) -> String {
    let (kind, feature, location) = match value {
        PptxWarning::UnsupportedTheme { part, name } => {
            ("UnsupportedTheme", name.as_str(), part.as_str())
        }
        PptxWarning::UnsupportedPresentationMetadata { part, name } => (
            "UnsupportedPresentationMetadata",
            name.as_str(),
            part.as_str(),
        ),
        PptxWarning::UnsupportedSlideContent { part, name, .. } => {
            ("UnsupportedSlideContent", name.as_str(), part.as_str())
        }
        PptxWarning::SlideContentNotRendered { part, .. } => {
            ("SlideContentNotRendered", "slide content", part.as_str())
        }
    };
    warning(kind, feature, location, &value.to_string())
}

fn convert(
    args: &[std::ffi::OsString],
    warnings: &mut Vec<String>,
    pages: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let package = o2p_ooxml::OoxmlPackage::open(&args[0], o2p_ooxml::SecurityLimits::default())?;
    let presentation =
        o2p_pptx::Presentation::parse_with_content(&package, o2p_pptx::ParseLimits::default())?;
    warnings.extend(presentation.warnings().iter().map(parser_warning));
    let display = if args.len() == 3 {
        presentation.to_display_list()
    } else {
        let mut book = o2p_fonts::FontBook::new();
        let mut fallbacks = Vec::new();
        for pair in args[3..].chunks(2) {
            match pair[0].to_str() {
                Some("--font-dir") => {
                    book.register_directory(&pair[1])?;
                }
                Some("--font") => {
                    book.register_file(&pair[1])?;
                }
                Some("--fallback-family") => fallbacks.push(
                    pair[1]
                        .to_str()
                        .ok_or("font family must be UTF-8")?
                        .to_owned(),
                ),
                _ => return Err("unknown font option".into()),
            }
        }
        if fallbacks.is_empty() {
            for font in book.fonts() {
                if !fallbacks.contains(&font.metadata().family) {
                    fallbacks.push(font.metadata().family.clone());
                }
            }
        }
        book.set_fallback_families(fallbacks);
        let layout = presentation.to_display_list_with_fonts(&book)?;
        for value in layout.warnings {
            let (kind, feature) = match &value.kind {
                TextLayoutWarningKind::Overflow => ("Overflow", "text box overflow".to_owned()),
                TextLayoutWarningKind::Font(o2p_fonts::FontWarning::MissingFont { family }) => {
                    ("MissingFont", family.clone())
                }
                TextLayoutWarningKind::Font(o2p_fonts::FontWarning::Substitution {
                    requested,
                    ..
                }) => ("Substitution", requested.family.clone()),
                TextLayoutWarningKind::Font(o2p_fonts::FontWarning::MissingGlyph { character }) => {
                    ("MissingGlyph", character.to_string())
                }
            };
            warnings.push(warning(
                kind,
                &feature,
                &format!("page {}, text box {}", value.page, value.text_box),
                &value.to_string(),
            ));
        }
        layout.display_list
    };
    for page in &display.pages {
        pages.push(format!(
            "{{\"width_pt\":{},\"height_pt\":{}}}",
            page.size.width, page.size.height
        ));
    }
    let report = o2p_pdf::render_with_report(&display)?;
    for value in report.warnings {
        warnings.push(warning(
            &format!("{:?}", value.kind),
            "missing glyph",
            &format!("page {}, operation {}", value.page, value.op),
            &value.to_string(),
        ));
    }
    fs::write(PathBuf::from(&args[1]), report.bytes)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() < 3 || (args.len() - 3) % 2 != 0 {
        eprintln!(
            "Usage: compat_render input.pptx output.pdf telemetry.json [--font-dir dir | --font file | --fallback-family name]..."
        );
        return ExitCode::from(2);
    }
    let start = Instant::now();
    let mut warnings = Vec::new();
    let mut pages = Vec::new();
    let result = convert(&args, &mut warnings, &mut pages);
    let error = result
        .as_ref()
        .err()
        .map(|e| quoted(&e.to_string()))
        .unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\"engine_version\":{},\"render_duration_seconds\":{},\"pages\":[{}],\"warnings\":[{}],\"error\":{}}}\n",
        quoted(env!("CARGO_PKG_VERSION")),
        start.elapsed().as_secs_f64(),
        pages.join(","),
        warnings.join(","),
        error
    );
    if let Err(error) = fs::write(PathBuf::from(&args[2]), json) {
        eprintln!("Cannot write telemetry: {error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = result {
        eprintln!("Cannot render: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_controls_quotes_backslashes_and_preserves_unicode() {
        assert_eq!(
            quoted("日本語\n\"\\\u{0}"),
            "\"日本語\\u000a\\\"\\\\\\u0000\""
        );
    }

    #[test]
    fn parser_adapter_retains_kind_feature_and_source_part() {
        let json = parser_warning(&PptxWarning::UnsupportedTheme {
            part: "ppt/theme/theme1.xml".into(),
            name: "gradFill".into(),
        });
        assert_eq!(
            json,
            "{\"kind\":\"UnsupportedTheme\",\"feature\":\"gradFill\",\"location\":\"ppt/theme/theme1.xml\",\"message\":\"ppt/theme/theme1.xml: unsupported theme gradFill\"}"
        );
    }
}
