//! Private Type0 / CIDFontType2 serialization adapter. No shaping or Office logic.
use crate::PdfError;
use o2p_core::{DisplayList, DrawOp, GlyphTextOp};
use o2p_fonts::FontFace;
use pdf_writer::{
    Content, Name, Pdf, Ref, Str,
    types::{CidFontType, FontFlags, SystemInfo},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(super) struct EmbeddedFont {
    pub name: String,
    pub reference: Ref,
    face: FontFace,
    codes: BTreeMap<(u16, String), u16>,
}
pub(super) type Fonts = BTreeMap<usize, EmbeddedFont>;
fn error(page: usize, op: usize, message: impl ToString) -> PdfError {
    PdfError::InvalidFont {
        page,
        op,
        message: message.to_string(),
    }
}

/// Source text is mapped once per shaping cluster, even for one-to-many glyphs.
fn glyph_codes(text: &GlyphTextOp, run: &o2p_core::GlyphRun) -> Vec<(u16, String)> {
    let mut seen = BTreeSet::new();
    run.glyphs
        .iter()
        .map(|glyph| {
            let source = if seen.insert((glyph.source.start, glyph.source.end)) {
                text.text.source[glyph.source.clone()].to_owned()
            } else {
                String::new()
            };
            (glyph.id, source)
        })
        .collect()
}
pub(super) fn prepare(display: &DisplayList, next: &mut i32) -> Result<Fonts, PdfError> {
    let mut fonts = Fonts::new();
    let mut total_bytes = 0;
    for (page, p) in display.pages.iter().enumerate() {
        for (op, operation) in p.ops.iter().enumerate() {
            let DrawOp::GlyphText(text) = operation else {
                continue;
            };
            if text.text.source.len() > o2p_fonts::MAX_TEXT_BYTES
                || text.text.runs.iter().map(|r| r.glyphs.len()).sum::<usize>()
                    > o2p_fonts::MAX_GLYPHS
            {
                return Err(error(page, op, "shaped text limit exceeded"));
            }
            if !text.text.source.is_empty()
                && text.text.runs.iter().all(|run| run.glyphs.is_empty())
            {
                return Err(error(page, op, "nonempty text has no glyph runs"));
            }
            for run in &text.text.runs {
                let key = Arc::as_ptr(&run.font) as usize;
                if !fonts.contains_key(&key) {
                    if fonts.len() >= 256
                        || run.font.bytes.len()
                            > (64 * 1024 * 1024usize).saturating_sub(total_bytes)
                    {
                        return Err(error(page, op, "embedded font resources limit exceeded"));
                    }
                    let face = FontFace::from_resource(run.font.clone())
                        .map_err(|e| error(page, op, e))?;
                    total_bytes += run.font.bytes.len();
                    let reference = Ref::new(*next);
                    *next = next.checked_add(6).ok_or(PdfError::TooManyPages)?;
                    fonts.insert(
                        key,
                        EmbeddedFont {
                            name: format!("E{}", reference.get()),
                            reference,
                            face,
                            codes: BTreeMap::new(),
                        },
                    );
                }
                let font = fonts
                    .get_mut(&key)
                    .ok_or_else(|| error(page, op, "missing registered resource"))?;
                for glyph in &run.glyphs {
                    if glyph.id >= font.face.metadata().glyph_count
                        || glyph.source.is_empty()
                        || text.text.source.get(glyph.source.clone()).is_none()
                    {
                        return Err(error(page, op, "invalid glyph ID or source range"));
                    }
                    if text.text.source[glyph.source.clone()]
                        .encode_utf16()
                        .count()
                        > 256
                    {
                        return Err(error(page, op, "Unicode cluster mapping limit exceeded"));
                    }
                }
                for code in glyph_codes(text, run) {
                    if !font.codes.contains_key(&code) {
                        let cid = u16::try_from(font.codes.len() + 1)
                            .map_err(|_| error(page, op, "PDF font CID limit exceeded"))?;
                        font.codes.insert(code, cid);
                    }
                }
            }
        }
    }
    Ok(fonts)
}
pub(super) fn write(pdf: &mut Pdf, fonts: &Fonts) {
    for font in ordered(fonts) {
        let base = font.face.metadata();
        let id = font.reference.get();
        let descendant = Ref::new(id + 1);
        let descriptor = Ref::new(id + 2);
        let program = Ref::new(id + 3);
        let cmap_id = Ref::new(id + 4);
        let gid_map_id = Ref::new(id + 5);
        let info = SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        };
        let base_name = Name(base.postscript_name.as_bytes());
        pdf.type0_font(font.reference)
            .base_font(base_name)
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(descendant)
            .to_unicode(cmap_id);
        let scale = 1000.0 / f32::from(base.units_per_em);
        let mut widths = vec![0.0; font.codes.len()];
        let mut gid_map = vec![0u8; (font.codes.len() + 1) * 2];

        for ((gid, _), &cid) in &font.codes {
            gid_map[usize::from(cid) * 2..usize::from(cid) * 2 + 2]
                .copy_from_slice(&gid.to_be_bytes());
            widths[usize::from(cid) - 1] =
                f32::from(font.face.glyph_advance(*gid).unwrap_or(0)) * scale;
        }
        {
            let mut cid = pdf.cid_font(descendant);
            cid.subtype(CidFontType::Type2)
                .base_font(base_name)
                .system_info(info)
                .font_descriptor(descriptor)
                .default_width(0.0)
                .cid_to_gid_map_stream(gid_map_id);
            cid.widths().consecutive(1, widths);
        }
        let bounds = base.bounds.map(|v| f32::from(v) * scale);
        pdf.font_descriptor(descriptor)
            .name(base_name)
            .flags(
                FontFlags::SYMBOLIC
                    | if base.italic {
                        FontFlags::ITALIC
                    } else {
                        FontFlags::empty()
                    },
            )
            .bbox(pdf_writer::Rect::new(
                bounds[0], bounds[1], bounds[2], bounds[3],
            ))
            .italic_angle(base.italic_angle)
            .ascent(f32::from(base.ascent) * scale)
            .descent(f32::from(base.descent) * scale)
            .cap_height(f32::from(base.cap_height) * scale)
            .stem_v(80.0)
            .font_file2(program);
        pdf.stream(program, &font.face.resource().bytes)
            .pair(Name(b"Length1"), font.face.resource().bytes.len() as i32);
        pdf.stream(cmap_id, &unicode_cmap(&font.codes));
        pdf.stream(gid_map_id, &gid_map);
    }
}
pub(super) fn ordered(fonts: &Fonts) -> Vec<&EmbeddedFont> {
    let mut list: Vec<_> = fonts.values().collect();
    list.sort_by_key(|f| f.reference.get());
    list
}
pub(super) fn draw(content: &mut Content, fonts: &Fonts, text: &GlyphTextOp, page_height: f32) {
    let actual: Vec<_> = [0xfe, 0xff]
        .into_iter()
        .chain(text.text.source.encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    content
        .begin_marked_content_with_properties(Name(b"Span"))
        .properties()
        .pair(Name(b"ActualText"), Str(&actual));
    crate::fill_color(content, text.color);
    content.begin_text();
    for run in &text.text.runs {
        let font = &fonts[&(Arc::as_ptr(&run.font) as usize)];
        content.set_font(Name(font.name.as_bytes()), text.text.font_size);
        let mut x = f64::from(text.origin.x) + f64::from(run.origin.x);
        let mut y = f64::from(text.origin.y) + f64::from(run.origin.y);
        for (glyph, key) in run.glyphs.iter().zip(glyph_codes(text, run)) {
            content.set_text_matrix([
                1.0,
                0.0,
                0.0,
                1.0,
                (x + f64::from(glyph.offset.x)) as f32,
                (f64::from(page_height) - y - f64::from(glyph.offset.y)) as f32,
            ]);
            content.show(Str(&font.codes[&key].to_be_bytes()));
            x += f64::from(glyph.advance.x);
            y += f64::from(glyph.advance.y);
        }
    }
    content.end_text();
    content.end_marked_content();
}

// ToUnicode is a type 2 CMap (not a glyph-encoding type 0 CMap).
fn unicode_cmap(codes: &BTreeMap<(u16, String), u16>) -> Vec<u8> {
    use std::fmt::Write;
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /O2PUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    // Continuation glyphs have no independent Unicode meaning. ActualText
    // supplies the complete logical source; omit an empty CMap destination.
    let pairs: Vec<_> = codes
        .iter()
        .filter(|((_, source), _)| !source.is_empty())
        .collect();
    for chunk in pairs.chunks(100) {
        // String formatting is infallible; no input-dependent indexing here.
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for ((_, source), cid) in chunk {
            let _ = write!(cmap, "<{cid:04X}> <");
            for unit in source.encode_utf16() {
                let _ = write!(cmap, "{unit:04X}");
            }
            cmap.push_str(">\n");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap.into_bytes()
}
