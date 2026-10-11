#![allow(dead_code)]
use flate2::{Compression, write::DeflateEncoder};
use std::io::Write;

pub const TYPES_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
pub const RELS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
pub const OFFICE_RELS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const MAIN_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
pub const SLIDE_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
pub const REL_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";

pub fn rels(inner: &str) -> Vec<u8> {
    format!("<?xml version='1.0' encoding='UTF-8'?><Relationships xmlns='{RELS_NS}'>{inner}</Relationships>").into_bytes()
}
pub fn relation(id: &str, kind: &str, target: &str) -> String {
    format!("<Relationship Id='{id}' Type='{OFFICE_RELS}/{kind}' Target='{target}'/>")
}
pub fn minimal(slides: usize, image: bool) -> Vec<(String, Vec<u8>)> {
    let mut types = format!(
        "<Types xmlns='{TYPES_NS}'><Default Extension='rels' ContentType='{REL_TYPE}'/><Default Extension='xml' ContentType='application/xml'/><Default Extension='dat' ContentType='application/octet-stream'/><Default Extension='png' ContentType='image/png'/><Override PartName='/ppt/presentation.xml' ContentType='{MAIN_TYPE}'/>"
    );
    let mut presentation_rels = String::new();
    let mut ids = String::new();
    let mut parts = Vec::new();
    for i in 1..=slides {
        types.push_str(&format!(
            "<Override PartName='/ppt/slides/slide{i}.xml' ContentType='{SLIDE_TYPE}'/>"
        ));
        presentation_rels.push_str(&relation(
            &format!("rId{i}"),
            "slide",
            &format!("slides/slide{i}.xml"),
        ));
        ids.push_str(&format!("<p:sldId id='{}' r:id='rId{i}'/>", 255 + i));
        let picture = if image && i == 1 {
            "<p:pic><p:nvPicPr><p:cNvPr id='2' name='Synthetic pixel'/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed='rImage'/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x='0' y='0'/><a:ext cx='914400' cy='914400'/></a:xfrm><a:prstGeom prst='rect'><a:avLst/></a:prstGeom></p:spPr></p:pic>"
        } else {
            ""
        };
        let slide = format!(
            "<p:sld xmlns:p='http://schemas.openxmlformats.org/presentationml/2006/main' xmlns:a='http://schemas.openxmlformats.org/drawingml/2006/main' xmlns:r='{OFFICE_RELS}'><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id='1' name=''/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{picture}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
        );
        parts.push((format!("ppt/slides/slide{i}.xml"), slide.into_bytes()));
    }
    types.push_str("</Types>");
    let presentation = format!(
        "<p:presentation xmlns:p='http://schemas.openxmlformats.org/presentationml/2006/main' xmlns:r='{OFFICE_RELS}'><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx='9144000' cy='6858000'/><p:notesSz cx='6858000' cy='9144000'/></p:presentation>"
    );
    parts.extend([
        ("[Content_Types].xml".into(), types.into_bytes()),
        (
            "_rels/.rels".into(),
            rels(&relation(
                "rOffice",
                "officeDocument",
                "ppt/presentation.xml",
            )),
        ),
        ("ppt/presentation.xml".into(), presentation.into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            rels(&presentation_rels),
        ),
    ]);
    if image {
        // Original synthetic one-pixel PNG, generated from raw RGBA with Rust/zlib.
        parts.push(("ppt/media/image1.png".into(), pixel_png()));
        parts.push(("ppt/slides/_rels/slide1.xml.rels".into(), rels(&format!("{}<Relationship Id='rLink' Type='{OFFICE_RELS}/hyperlink' Target='https://example.invalid/never-fetch' TargetMode='External'/>", relation("rImage", "image", "../media/image1.png")))));
    }
    parts
}

fn pixel_png() -> Vec<u8> {
    fn chunk(output: &mut Vec<u8>, name: &[u8], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(data);
        let mut crc = crc32fast::Hasher::new();
        crc.update(name);
        crc.update(data);
        output.extend_from_slice(&crc.finalize().to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut png, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&[0, 30, 100, 160, 255]).unwrap();
    chunk(&mut png, b"IDAT", &encoder.finish().unwrap());
    chunk(&mut png, b"IEND", &[]);
    png
}

/// Deterministic ZIP32 writer for test data only; no production writer API.
pub fn zip(parts: &[(String, Vec<u8>)], deflate: bool, descriptor: bool) -> Vec<u8> {
    fn u16w(out: &mut Vec<u8>, n: u16) {
        out.extend_from_slice(&n.to_le_bytes());
    }
    fn u32w(out: &mut Vec<u8>, n: u32) {
        out.extend_from_slice(&n.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, bytes) in parts {
        let local_offset = out.len() as u32;
        let crc = crc32fast::hash(bytes);
        let data = if deflate {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(bytes).unwrap();
            encoder.finish().unwrap()
        } else {
            bytes.clone()
        };
        let flags = 0x800 | if descriptor { 8 } else { 0 };
        let method = if deflate { 8 } else { 0 };
        out.extend_from_slice(b"PK\x03\x04");
        u16w(&mut out, 20);
        u16w(&mut out, flags);
        u16w(&mut out, method);
        u16w(&mut out, 0);
        u16w(&mut out, 33); // 1980-01-01, no clock dependency
        for value in [crc, data.len() as u32, bytes.len() as u32] {
            u32w(&mut out, if descriptor { 0 } else { value });
        }
        u16w(&mut out, name.len() as u16);
        u16w(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&data);
        if descriptor {
            out.extend_from_slice(b"PK\x07\x08");
            for value in [crc, data.len() as u32, bytes.len() as u32] {
                u32w(&mut out, value);
            }
        }
        directory.extend_from_slice(b"PK\x01\x02");
        u16w(&mut directory, 20);
        u16w(&mut directory, 20);
        u16w(&mut directory, flags);
        u16w(&mut directory, method);
        u16w(&mut directory, 0);
        u16w(&mut directory, 33);
        for value in [crc, data.len() as u32, bytes.len() as u32] {
            u32w(&mut directory, value);
        }
        u16w(&mut directory, name.len() as u16);
        for _ in 0..4 {
            u16w(&mut directory, 0);
        }
        u32w(&mut directory, 0);
        u32w(&mut directory, local_offset);
        directory.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&directory);
    out.extend_from_slice(b"PK\x05\x06");
    u16w(&mut out, 0);
    u16w(&mut out, 0);
    u16w(&mut out, parts.len() as u16);
    u16w(&mut out, parts.len() as u16);
    u32w(&mut out, directory.len() as u32);
    u32w(&mut out, start);
    u16w(&mut out, 0);
    out
}
