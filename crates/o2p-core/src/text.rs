//! Deterministic advances of the fixed sans-serif fallback, in ems.
//! These are unkerned Helvetica-compatible metrics, not a font resolver.
use crate::Font;

impl Font {
    pub fn sans_serif(bold: bool, italic: bool) -> Self {
        match (bold, italic) {
            (false, false) => Self::SansSerif,
            (true, false) => Self::SansSerifBold,
            (false, true) => Self::SansSerifItalic,
            (true, true) => Self::SansSerifBoldItalic,
        }
    }

    /// Measure a single line using fixed fallback metrics, without kerning.
    /// Unsupported glyphs use the advance of `?`, matching the reporting renderer.
    /// The caller applies the positive font size; accumulation uses f64.
    pub fn text_width(self, text: &str, font_size: f32) -> f64 {
        text.chars()
            .map(|c| f64::from(self.advance(c)))
            .sum::<f64>()
            * f64::from(font_size)
            / 1000.0
    }

    fn advance(self, character: char) -> u16 {
        let bold = matches!(self, Self::SansSerifBold | Self::SansSerifBoldItalic);
        let ascii = match character {
            '\u{a0}' => ' ',
            '\u{ad}' => '-',
            'À'..='Å' => 'A',
            'Ç' => 'C',
            'È'..='Ë' => 'E',
            'Ì'..='Ï' => 'I',
            'Ð' => 'D',
            'Ñ' => 'N',
            'Ò'..='Ö' | 'Ø' => 'O',
            'Ù'..='Ü' => 'U',
            'Ý' => 'Y',
            'à'..='å' => 'a',
            'ç' => 'c',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ñ' => 'n',
            'ò'..='ö' | 'ø' => 'o',
            'ù'..='ü' => 'u',
            'ý' | 'ÿ' => 'y',
            'Š' => 'S',
            'š' => 's',
            'Ž' => 'Z',
            'ž' => 'z',
            'Ÿ' => 'Y',
            '\u{20}'..='\u{7e}' => character,
            _ => {
                return match character {
                    '¡' => 333,
                    '¢' | '£' | '¤' | '¥' | '§' | '¶' | '€' => 556,
                    '¦' => 260,
                    '¨' | '¯' | '´' | '¸' => 333,
                    '©' | '®' => 737,
                    'ª' | 'º' => 370,
                    '«' | '»' => 556,
                    '¬' | '±' | '×' | '÷' => 584,
                    '°' => 400,
                    '²' | '³' | '¹' => 333,
                    'µ' => {
                        if bold {
                            611
                        } else {
                            556
                        }
                    }
                    '·' => 278,
                    '¼' | '½' | '¾' => 834,
                    '¿' => 611,
                    'Æ' => 1000,
                    'æ' => 889,
                    'Þ' => 667,
                    'þ' => {
                        if bold {
                            611
                        } else {
                            556
                        }
                    }
                    'ð' | 'ß' => {
                        if bold {
                            611
                        } else {
                            556
                        }
                    }
                    'Œ' => 1000,
                    'œ' => 944,
                    'ƒ' => 556,
                    'ˆ' | '˜' => 333,
                    '‘' | '’' | '‚' => {
                        if bold {
                            278
                        } else {
                            222
                        }
                    }
                    '“' | '”' | '„' => {
                        if bold {
                            500
                        } else {
                            333
                        }
                    }
                    '†' | '‡' => 556,
                    '•' => 350,
                    '…' | '‰' | '—' | '™' => 1000,
                    '–' => 556,
                    '‹' | '›' => 333,
                    _ => {
                        if bold {
                            611
                        } else {
                            556
                        }
                    }
                };
            }
        };
        let widths = if bold { &BOLD } else { &REGULAR };
        widths[ascii as usize - 32]
    }
}

const REGULAR: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_and_faces_match_the_fixed_fallback() {
        assert_eq!(Font::sans_serif(true, true), Font::SansSerifBoldItalic);
        assert_eq!(Font::sans_serif(false, true), Font::SansSerifItalic);
        assert!((Font::SansSerif.text_width("Hello", 10.0) - 22.78).abs() < 1e-6);
        assert_eq!(
            Font::SansSerif.text_width("é", 10.0),
            Font::SansSerif.text_width("e", 10.0)
        );
        assert_eq!(
            Font::SansSerif.text_width("日本語", 10.0),
            Font::SansSerif.text_width("???", 10.0)
        );
    }
}
