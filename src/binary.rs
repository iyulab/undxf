//! Binary DXF, written out as the ASCII DXF it stands for.
//!
//! A binary DXF holds the same groups as an ASCII one, each code and value in
//! binary: after the 22-byte sentinel, every group is its code -- one byte
//! up to R12 (255 escaping a two-byte code), two bytes (little-endian) from
//! R13 -- followed by its value, whose form the code's range fixes: a double
//! (8 bytes), a 16-, 32- or 64-bit integer, a one-byte flag, a length-prefixed
//! chunk of bytes, or a zero-terminated string. The format is described in
//! the DXF reference ("Binary DXF Files"); the value ranges are the ones the
//! reference's group code table gives each type.
//!
//! This module writes each group as the two lines the ASCII form has for it,
//! so the rest of the crate reads one form. A number is written so that it
//! reads back as the same value; a string keeps its bytes, which are decoded
//! with the rest of the text; a byte chunk is written in hexadecimal, as the
//! ASCII form writes it. A string that holds a line break or another control
//! character, or a caret, is written in the caret notation the ASCII form uses
//! for them (`^J`, `^ `), which the crate undoes like any other string's.

use crate::pairs::ReadError;

/// What a binary DXF starts with.
pub const SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\x00";

enum Kind {
    Double,
    Int16,
    Int32,
    Int64,
    Flag,
    Chunk,
    Text,
}

fn kind(code: i32) -> Kind {
    match code {
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => Kind::Double,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => Kind::Int16,
        90..=99 | 420..=429 | 440..=459 | 1071 => Kind::Int32,
        160..=169 => Kind::Int64,
        290..=299 => Kind::Flag,
        310..=319 | 1004 => Kind::Chunk,
        _ => Kind::Text,
    }
}

/// The ASCII DXF bytes `bytes` (a binary DXF, sentinel included) stand for.
pub fn to_ascii(bytes: &[u8]) -> Result<Vec<u8>, ReadError> {
    let data = bytes
        .strip_prefix(SENTINEL)
        .ok_or_else(|| error(0, "not a binary DXF (no sentinel)"))?;
    // The first group is 0/SECTION: a one-byte code is followed by the `S`
    // of SECTION, a two-byte one by the code's second byte, zero.
    let wide = match data {
        [0, 0, ..] => true,
        [0, b'S', ..] => false,
        _ => return Err(error(SENTINEL.len(), "the first group is not 0/SECTION")),
    };
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut i = 0;
    while i < data.len() {
        let at = SENTINEL.len() + i;
        let code = if wide {
            let c = take::<2>(data, &mut i, at)?;
            i32::from(u16::from_le_bytes(c))
        } else {
            let [c] = take::<1>(data, &mut i, at)?;
            if c == 255 {
                i32::from(u16::from_le_bytes(take::<2>(data, &mut i, at)?))
            } else {
                i32::from(c)
            }
        };
        out.extend_from_slice(format!("{code:>3}\n").as_bytes());
        match kind(code) {
            Kind::Double => {
                let v = f64::from_le_bytes(take::<8>(data, &mut i, at)?);
                out.extend_from_slice(format!("{v:?}").as_bytes());
            }
            Kind::Int16 => {
                let v = i16::from_le_bytes(take::<2>(data, &mut i, at)?);
                out.extend_from_slice(v.to_string().as_bytes());
            }
            Kind::Int32 => {
                let v = i32::from_le_bytes(take::<4>(data, &mut i, at)?);
                out.extend_from_slice(v.to_string().as_bytes());
            }
            Kind::Int64 => {
                let v = i64::from_le_bytes(take::<8>(data, &mut i, at)?);
                out.extend_from_slice(v.to_string().as_bytes());
            }
            Kind::Flag => {
                let [v] = take::<1>(data, &mut i, at)?;
                out.extend_from_slice(v.to_string().as_bytes());
            }
            Kind::Chunk => {
                let [len] = take::<1>(data, &mut i, at)?;
                let chunk = data
                    .get(i..i + usize::from(len))
                    .ok_or_else(|| error(at, "a byte chunk runs past the end of the file"))?;
                i += usize::from(len);
                for b in chunk {
                    out.extend_from_slice(format!("{b:02X}").as_bytes());
                }
            }
            Kind::Text => {
                let len = data[i..]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or_else(|| error(at, "a string has no terminating zero"))?;
                for &b in &data[i..i + len] {
                    match b {
                        b'^' => out.extend_from_slice(b"^ "),
                        0x01..=0x1F => out.extend_from_slice(&[b'^', b + 0x40]),
                        _ => out.push(b),
                    }
                }
                i += len + 1;
            }
        }
        out.push(b'\n');
    }
    Ok(out)
}

fn take<const N: usize>(data: &[u8], i: &mut usize, at: usize) -> Result<[u8; N], ReadError> {
    let bytes: [u8; N] = data
        .get(*i..*i + N)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| error(at, "the file ends inside a group"))?;
    *i += N;
    Ok(bytes)
}

fn error(offset: usize, detail: &str) -> ReadError {
    ReadError::Binary {
        offset,
        detail: detail.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(groups: &[(u16, &[u8])]) -> Vec<u8> {
        let mut b = SENTINEL.to_vec();
        for (code, value) in groups {
            b.extend_from_slice(&code.to_le_bytes());
            b.extend_from_slice(value);
        }
        b
    }

    #[test]
    fn each_group_becomes_its_two_ascii_lines() {
        let bytes = wide(&[
            (0, b"SECTION\0"),
            (2, b"ENTITIES\0"),
            (10, &1.5f64.to_le_bytes()),
            (70, &(-3i16).to_le_bytes()),
            (90, &70000i32.to_le_bytes()),
            (290, &[1]),
            (310, &[2, 0xAB, 0x01]),
            (0, b"EOF\0"),
        ]);
        let text = String::from_utf8(to_ascii(&bytes).unwrap()).unwrap();
        assert_eq!(
            text,
            "  0\nSECTION\n  2\nENTITIES\n 10\n1.5\n 70\n-3\n 90\n70000\n290\n1\n310\nAB01\n  0\nEOF\n"
        );
    }

    #[test]
    fn a_double_reads_back_as_the_same_value() {
        for v in [0.1, 4235.406760796846, 1e20, -1e-300, f64::MAX] {
            let bytes = wide(&[(0, b"SECTION\0"), (10, &v.to_le_bytes())]);
            let text = String::from_utf8(to_ascii(&bytes).unwrap()).unwrap();
            let line = text.lines().nth(3).unwrap();
            assert_eq!(
                line.parse::<f64>().unwrap().to_bits(),
                v.to_bits(),
                "{line}"
            );
        }
    }

    #[test]
    fn one_byte_codes_escape_to_two_with_255() {
        let mut b = SENTINEL.to_vec();
        b.extend_from_slice(b"\0SECTION\0");
        b.push(255);
        b.extend_from_slice(&1001u16.to_le_bytes());
        b.extend_from_slice(b"ACAD\0");
        let text = String::from_utf8(to_ascii(&b).unwrap()).unwrap();
        assert_eq!(text, "  0\nSECTION\n1001\nACAD\n");
    }

    #[test]
    fn a_control_character_or_caret_in_a_string_takes_the_caret_notation() {
        let bytes = wide(&[(0, b"SECTION\0"), (1, b"a\nb^c\0")]);
        let text = String::from_utf8(to_ascii(&bytes).unwrap()).unwrap();
        assert_eq!(text, "  0\nSECTION\n  1\na^Jb^ c\n");
    }

    #[test]
    fn a_file_that_ends_inside_a_group_is_an_error_with_its_offset() {
        let mut bytes = wide(&[(0, b"SECTION\0")]);
        bytes.extend_from_slice(&10u16.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0]);
        let err = to_ascii(&bytes).unwrap_err();
        assert!(
            matches!(err, ReadError::Binary { offset, .. } if offset == SENTINEL.len() + 10),
            "{err:?}"
        );
    }
}
