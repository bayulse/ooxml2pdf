//! Explicit development diagnostic; normal conversion never prints layout data.
use o2p_fonts::FontBook;
use o2p_pptx::{Presentation, Shape};
use std::{error::Error, path::PathBuf};

fn trace(shapes: &[Shape], fonts: &FontBook, index: &mut usize) -> Result<(), Box<dyn Error>> {
    for shape in shapes {
        match shape {
            Shape::Transformed { element, transform } => {
                println!("transform={transform:?}");
                trace(element, fonts, index)?;
            }
            Shape::Shadowed { element, .. } => trace(std::slice::from_ref(element), fonts, index)?,
            Shape::TextBox(text) => {
                println!(
                    "textbox={} rect={:?} insets={:?} anchor={:?}",
                    *index, text.rect, text.insets, text.vertical_alignment
                );
                println!(
                    "inner_y={} inner_height={}",
                    (f64::from(text.rect.origin.y) + f64::from(text.insets.top)) as f32,
                    (f64::from(text.rect.size.height)
                        - f64::from(text.insets.top)
                        - f64::from(text.insets.bottom))
                    .max(0.0) as f32
                );
                for paragraph in &text.paragraphs {
                    for run in &paragraph.runs {
                        let request = o2p_fonts::FontRequest {
                            family: run
                                .style
                                .family
                                .clone()
                                .unwrap_or_else(|| "sans-serif".into()),
                            weight: if run.style.bold { 700 } else { 400 },
                            italic: run.style.italic,
                        };
                        let shaped =
                            o2p_fonts::shape(fonts, &run.text, &request, run.style.font_size)?;
                        println!(
                            "text={:?} request={request:?} size={} metrics={:?}",
                            run.text, run.style.font_size, shaped.metrics
                        );
                        for segment in &shaped.text.runs {
                            if let Some(selected) = fonts
                                .fonts()
                                .iter()
                                .find(|face| std::sync::Arc::ptr_eq(face.resource(), &segment.font))
                            {
                                println!(
                                    "selected={} raw_metrics={:?}",
                                    selected.metadata().family,
                                    selected.metadata().vertical_metrics
                                );
                            }
                        }
                    }
                }
                let layout = text.measure_with_fonts(fonts)?;
                println!(
                    "line_count={} block_height={} overflow={}",
                    layout.lines.len(),
                    layout.height,
                    layout.overflow
                );
                for (i, line) in layout.lines.iter().enumerate() {
                    println!(
                        "line={} top={} baseline={:?} bottom={} height={} width={} metrics={:?}",
                        i,
                        line.top,
                        line.baseline,
                        line.bottom,
                        line.height,
                        line.width,
                        line.metrics
                    );
                    for run in &line.runs {
                        println!("origin={:?} advance={:?}", run.origin, run.text.advance);
                    }
                }
                *index += 1;
            }
            _ => {}
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || (args.len() - 1) % 2 != 0 {
        return Err("Usage: text_layout_trace input.pptx [--font file | --font-dir dir | --fallback-family name]...".into());
    }
    let mut fonts = FontBook::new();
    let mut fallbacks = Vec::new();
    for pair in args[1..].chunks_exact(2) {
        match pair[0].to_str() {
            Some("--font") => {
                fonts.register_file(&pair[1])?;
            }
            Some("--font-dir") => {
                fonts.register_directory(&pair[1])?;
            }
            Some("--fallback-family") => {
                fallbacks.push(pair[1].to_str().ok_or("family must be UTF-8")?.to_owned())
            }
            _ => return Err("unknown font option".into()),
        }
    }
    if fallbacks.is_empty() {
        for font in fonts.fonts() {
            if !fallbacks.contains(&font.metadata().family) {
                fallbacks.push(font.metadata().family.clone());
            }
        }
    }
    fonts.set_fallback_families(fallbacks);
    let package = o2p_ooxml::OoxmlPackage::open(PathBuf::from(&args[0]), Default::default())?;
    let presentation = Presentation::parse_with_content(&package, Default::default())?;
    for (i, slide) in presentation.slide_contents().iter().enumerate() {
        println!("slide={}", i + 1);
        trace(slide.shapes(), &fonts, &mut 0)?;
    }
    Ok(())
}
