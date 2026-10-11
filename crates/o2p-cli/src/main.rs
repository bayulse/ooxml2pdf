use std::{env, ffi::OsString, fs, path::PathBuf, process::ExitCode};

use o2p_core::{Color, DisplayList, DrawOp, Font, LineOp, Page, Point, Rect, RectOp, Size, TextOp};

const USAGE: &str =
    "Usage: o2p demo <output.pdf>\n       o2p inspect <file.pptx>\n       o2p --help";

enum Command {
    Demo(PathBuf),
    Inspect(PathBuf),
    Help,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    match (args.next(), args.next(), args.next()) {
        (Some(command), None, None) if command == "--help" || command == "-h" => Ok(Command::Help),
        (Some(command), Some(path), None) if command == "demo" && !path.is_empty() => {
            Ok(Command::Demo(path.into()))
        }
        (Some(command), Some(path), None) if command == "inspect" && !path.is_empty() => {
            Ok(Command::Inspect(path.into()))
        }
        _ => Err(USAGE),
    }
}

fn demo() -> DisplayList {
    DisplayList {
        pages: vec![Page {
            size: Size {
                width: 595.0,
                height: 842.0,
            },
            ops: vec![
                DrawOp::Rect(RectOp {
                    rect: Rect {
                        origin: Point { x: 48.0, y: 48.0 },
                        size: Size {
                            width: 499.0,
                            height: 112.0,
                        },
                    },
                    color: Color::rgb(225, 238, 250),
                }),
                DrawOp::Text(TextOp {
                    origin: Point { x: 64.0, y: 92.0 },
                    text: "ooxml2pdf - Milestone 0".into(),
                    font: Font::SansSerif,
                    font_size: 24.0,
                    color: Color::rgb(20, 45, 70),
                }),
                DrawOp::Text(TextOp {
                    origin: Point { x: 64.0, y: 128.0 },
                    text: "Common DisplayList -> PDF (pure Rust, offline)".into(),
                    font: Font::SansSerif,
                    font_size: 12.0,
                    color: Color::BLACK,
                }),
                DrawOp::Line(LineOp {
                    start: Point { x: 48.0, y: 184.0 },
                    end: Point { x: 547.0, y: 184.0 },
                    width: 2.0,
                    color: Color::rgb(30, 100, 160),
                }),
            ],
        }],
    }
}

fn main() -> ExitCode {
    let command = match parse_args(env::args_os().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => println!("{USAGE}"),
        Command::Inspect(path) => {
            let inspection =
                match o2p_ooxml::OoxmlPackage::open(&path, o2p_ooxml::SecurityLimits::default())
                    .and_then(|package| package.inspect_pptx())
                {
                    Ok(inspection) => inspection,
                    Err(error) => {
                        eprintln!("Cannot inspect {}: {error}", path.display());
                        return ExitCode::FAILURE;
                    }
                };
            println!(
                "Format: PPTX\nSlides: {}\nMedia: {}",
                inspection.slide_count(),
                inspection.media_count()
            );
            println!("\nSlides:");
            for slide in inspection.slides {
                println!("- {slide}");
            }
            println!("\nMedia:");
            for medium in inspection.media {
                println!("- {medium}");
            }
        }
        Command::Demo(path) => {
            let bytes = match o2p_pdf::render(&demo()) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("PDF rendering failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            if let Err(error) = fs::write(&path, bytes) {
                eprintln!("Cannot write {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
            println!("Wrote {}", path.display());
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_demo_and_help() {
        assert!(
            matches!(parse_args(["demo".into(), "file.pdf".into()]), Ok(Command::Demo(path)) if path == std::path::Path::new("file.pdf"))
        );
        assert!(matches!(parse_args(["--help".into()]), Ok(Command::Help)));
        assert!(matches!(
            parse_args(["inspect".into(), "file.pptx".into()]),
            Ok(Command::Inspect(_))
        ));
    }

    #[test]
    fn rejects_missing_unknown_and_extra_arguments() {
        for args in [
            vec![],
            vec!["demo"],
            vec!["convert", "file.pdf"],
            vec!["demo", "file.pdf", "extra"],
            vec!["--help", "extra"],
            vec!["demo", ""],
            vec!["inspect"],
            vec!["inspect", ""],
            vec!["inspect", "file.pptx", "extra"],
        ] {
            assert!(parse_args(args.into_iter().map(OsString::from)).is_err());
        }
    }
}
