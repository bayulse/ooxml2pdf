//! Private adapter for the fixed PDF standard Helvetica faces and WinAnsi.
use o2p_core::Font;
use pdf_writer::Name;

pub(super) fn face(font: Font) -> (Name<'static>, &'static [u8], usize) {
    match font {
        Font::SansSerif => (Name(b"F1"), b"Helvetica", 0),
        Font::SansSerifBold => (Name(b"F2"), b"Helvetica-Bold", 1),
        Font::SansSerifItalic => (Name(b"F3"), b"Helvetica-Oblique", 2),
        Font::SansSerifBoldItalic => (Name(b"F4"), b"Helvetica-BoldOblique", 3),
    }
}

pub(super) fn encode(character: char) -> Option<u8> {
    match character {
        ' '..='~' | '\u{a0}'..='\u{ff}' => Some(character as u8),
        _ => EXTRA
            .iter()
            .find_map(|&(c, byte)| (c == character).then_some(byte)),
    }
}

const EXTRA: [(char, u8); 27] = [
    ('€', 0x80),
    ('‚', 0x82),
    ('ƒ', 0x83),
    ('„', 0x84),
    ('…', 0x85),
    ('†', 0x86),
    ('‡', 0x87),
    ('ˆ', 0x88),
    ('‰', 0x89),
    ('Š', 0x8a),
    ('‹', 0x8b),
    ('Œ', 0x8c),
    ('Ž', 0x8e),
    ('‘', 0x91),
    ('’', 0x92),
    ('“', 0x93),
    ('”', 0x94),
    ('•', 0x95),
    ('–', 0x96),
    ('—', 0x97),
    ('˜', 0x98),
    ('™', 0x99),
    ('š', 0x9a),
    ('›', 0x9b),
    ('œ', 0x9c),
    ('ž', 0x9e),
    ('Ÿ', 0x9f),
];
