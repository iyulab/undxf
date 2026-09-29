//! The SAT text of a 3DSOLID's or a REGION's ACIS body, as a DXF record
//! stores it.
//!
//! Up to R2010 the body is in the record itself: each line of SAT text starts
//! at a group 1, and a line too long for one group continues in the group 3s
//! that follow it. The text is obfuscated character by character: a space
//! stays a space, `@` and `_` swap places, the characters `A` to `^` (0x41 to
//! 0x5E) are written in reverse order of that range -- `^` means `A`, and an
//! `A` is written as `^` followed by a space, which is not part of the text --
//! and every other character is written XOR 0x5F. From R2013 the body is in
//! the `ACDSDATA` section, in ACIS's binary form (SAB), and the record has no
//! group 1 at all -- the reader takes it from there.

use crate::pairs::Pair;

/// The SAT text in `pairs`, one line per group 1 with its group 3s appended,
/// undone; `None` when the record carries no group 1 (the body is stored
/// elsewhere, or there is none).
pub fn sat_text(pairs: &[Pair<'_>]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for p in pairs {
        match p.code {
            1 => lines.push(p.value.to_string()),
            3 => {
                if let Some(line) = lines.last_mut() {
                    line.push_str(p.value);
                }
            }
            _ => {}
        }
    }
    if lines.is_empty() {
        return None;
    }
    let decoded: Vec<String> = lines.iter().map(|l| decode(l)).collect();
    Some(decoded.join("\n"))
}

/// One line of obfuscated SAT text, undone.
fn decode(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut skip = false;
    for c in line.chars() {
        if skip {
            skip = false;
            continue;
        }
        let code = c as u32;
        let decoded = match code {
            0x20 => ' ',
            0x40 => '_',
            0x5F => '@',
            0x41..=0x5E => {
                skip = code == 0x5E;
                char::from_u32(0x41 + (0x5E - code)).unwrap_or(c)
            }
            0..=0x7F => char::from_u32(code ^ 0x5F).unwrap_or(c),
            _ => c,
        };
        out.push(decoded);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(code: i32, value: &str) -> Pair<'_> {
        Pair {
            code,
            value,
            line: 0,
        }
    }

    #[test]
    fn digits_and_spaces_undo_by_xor() {
        // The first line of a version-400 body as a DXF file writes it.
        assert_eq!(decode("koo nll n o"), "400 133 1 0");
    }

    #[test]
    fn letters_run_in_reverse_and_an_a_drops_the_space_after_it() {
        // `^` + space is `A`; `]` is `B`; `@` and `_` swap.
        assert_eq!(decode("^  ]"), "A B");
        assert_eq!(decode("@_"), "_@");
        // Lower case is outside the reversed range: XOR, like a digit.
        assert_eq!(decode("+"), "t");
    }

    #[test]
    fn a_group_3_continues_the_line_its_group_1_started() {
        let pairs = [
            pair(100, "AcDbModelerGeometry"),
            pair(70, "1"),
            pair(1, "ko"),
            pair(3, "o"),
            pair(1, "n"),
        ];
        assert_eq!(sat_text(&pairs).as_deref(), Some("400\n1"));
    }

    #[test]
    fn a_record_without_group_1_has_no_text() {
        let pairs = [
            pair(100, "AcDbModelerGeometry"),
            pair(290, "1"),
            pair(2, "{7cf5f000-46fc-4a48-beef-14b47a01fb47}"),
        ];
        assert_eq!(sat_text(&pairs), None);
    }
}
