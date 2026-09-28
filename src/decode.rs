//! From the file's bytes to text: which code page the header declares, and
//! what to do with it.
//!
//! A DXF file from before R2007 stores its strings in the 8-bit code page
//! its header names (`$DWGCODEPAGE`); from R2007 on, strings are UTF-8.
//! Bytes that are valid UTF-8 are taken as UTF-8 whatever the header says
//! -- an ASCII-only file is both, and a file written by a tool that puts
//! UTF-8 text under a legacy header reads right that way. Anything else is
//! decoded through the declared code page -- or, when a file from before
//! R2007 declares none, through ANSI_1252, the code page readers of the
//! format assume then, and said so -- and what could not be decoded is said
//! in the drawing's diagnostics, never dropped in silence.

use crate::pairs::pairs;
use encoding_rs::Encoding;

/// The version from which DXF text is UTF-8 (R2007).
const UTF8_FROM: &str = "AC1021";

/// How far into the file the HEADER section is looked for.
const HEADER_SCAN: usize = 64 * 1024;

/// The text of `bytes`, and the warnings the decoding raised (empty when
/// every byte read as the file intended).
pub fn decode(bytes: &[u8]) -> (String, Vec<String>) {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (text.to_string(), Vec::new());
    }
    let (version, codepage) = header(bytes);
    let mut warnings = Vec::new();
    if version.as_deref().is_some_and(|v| v >= UTF8_FROM) {
        warnings.push("TEXT_ENCODING: the file declares a version whose text is UTF-8, but its bytes are not valid UTF-8; invalid sequences read as U+FFFD".to_string());
        return (String::from_utf8_lossy(bytes).into_owned(), warnings);
    }
    let name = match codepage {
        Some(name) => name,
        None => {
            warnings.push(format!(
                "CODEPAGE_ASSUMED: the file's bytes are not valid UTF-8 and its header names no code page; its text was read as {ASSUMED}"
            ));
            ASSUMED.to_string()
        }
    };
    let Some(encoding) = encoding_for(&name) else {
        warnings.push(format!(
            "CODEPAGE_UNSUPPORTED: {name}; the file's text was read as UTF-8 and its non-ASCII strings are wrong"
        ));
        return (String::from_utf8_lossy(bytes).into_owned(), warnings);
    };
    let (text, mut had_errors) = encoding.decode_without_bom_handling(bytes);
    let mut text = text.into_owned();
    // A single-byte code page's unassigned bytes come back as C1 control
    // characters (U+0080-U+009F) from the decoder, which follows the web's
    // encoding standard there. No drawing's text means one: they are the
    // bytes the code page has no character for.
    if encoding.is_single_byte() && text.chars().any(is_c1) {
        text = text
            .chars()
            .map(|c| if is_c1(c) { '\u{fffd}' } else { c })
            .collect();
        had_errors = true;
    }
    if had_errors {
        warnings.push(format!(
            "TEXT_ENCODING: bytes that code page {name} has no character for read as U+FFFD"
        ));
    }
    (text, warnings)
}

/// The code page a file from before R2007 that names none is read in.
const ASSUMED: &str = "ANSI_1252";

fn is_c1(c: char) -> bool {
    ('\u{80}'..='\u{9f}').contains(&c)
}

/// `$ACADVER` and `$DWGCODEPAGE` from the HEADER section, when present.
fn header(bytes: &[u8]) -> (Option<String>, Option<String>) {
    let head = &bytes[..bytes.len().min(HEADER_SCAN)];
    let text = String::from_utf8_lossy(head);
    let Ok(pairs) = pairs(&text) else {
        return (None, None);
    };
    let mut version = None;
    let mut codepage = None;
    let mut i = 0;
    while i + 1 < pairs.len() {
        let (p, v) = (pairs[i], pairs[i + 1]);
        if p.code == 0 && p.value == "ENDSEC" {
            break;
        }
        if p.code == 9 {
            match p.value {
                "$ACADVER" => version = Some(v.value.trim().to_string()),
                "$DWGCODEPAGE" => codepage = Some(v.value.trim().to_string()),
                _ => {}
            }
        }
        i += 1;
    }
    (version, codepage)
}

/// A string value as the model carries it: the file's storage of it undone
/// -- its `\U+` / `\M+` escapes and its caret notation (principles §6.1,
/// [`uncad_model::text`]). Every string this crate puts into the model goes
/// through here, names as well as text, so that a name an entity refers by
/// and the table entry it names are undone alike.
pub(crate) fn string(value: &str) -> String {
    let escaped = uncad_model::text::decode_escapes(value, multibyte);
    uncad_model::text::decode_caret(&escaped).into_owned()
}

/// A `\M+` escape's two bytes in the code page it names.
fn multibyte(codepage: u16, bytes: [u8; 2]) -> Option<char> {
    let encoding = encoding_for(&format!("ANSI_{codepage}"))?;
    let (text, had_errors) = encoding.decode_without_bom_handling(&bytes);
    let mut chars = text.chars();
    match (had_errors, chars.next(), chars.next()) {
        (false, Some(c), None) => Some(c),
        _ => None,
    }
}

/// The encoding a `$DWGCODEPAGE` name means; case does not matter. The
/// names are the `ANSI_` forms the DXF reference lists (`DOS` for the same
/// numbers), and the other names files carry for a code page: `CP932`,
/// `CP949`, `BIG5`, `GB2312`, `CP866`, `MACINTOSH`, the `ISO-8859-` family,
/// `US_ASCII` and `UTF8`. A name is read the way the web's encoding standard
/// reads its label, so `ISO-8859-1` and `US_ASCII` are windows-1252, of
/// which they are subsets. The DOS-era code pages other than 866 (`CP437`,
/// `CP850`, ...) and Johab have no decoder here.
fn encoding_for(name: &str) -> Option<&'static Encoding> {
    let upper = name.to_ascii_uppercase();
    let windows = |number: &str| -> Option<&'static str> {
        Some(match number {
            "932" => "shift_jis",
            "936" => "gbk",
            "949" => "euc-kr",
            "950" => "big5",
            "874" => "windows-874",
            "1250" => "windows-1250",
            "1251" => "windows-1251",
            "1252" => "windows-1252",
            "1253" => "windows-1253",
            "1254" => "windows-1254",
            "1255" => "windows-1255",
            "1256" => "windows-1256",
            "1257" => "windows-1257",
            "1258" => "windows-1258",
            _ => return None,
        })
    };
    let label = if let Some(number) = upper
        .strip_prefix("ANSI_")
        .or_else(|| upper.strip_prefix("DOS"))
    {
        windows(number)?
    } else {
        match upper.as_str() {
            "CP932" => "shift_jis",
            "CP949" => "euc-kr",
            "CP866" => "ibm866",
            "BIG5" => "big5",
            "GB2312" => "gbk",
            "MACINTOSH" => "macintosh",
            "US_ASCII" => "us-ascii",
            "UTF8" => "utf-8",
            iso if iso.starts_with("ISO-8859-") => iso,
            _ => return None,
        }
    };
    Encoding::for_label(label.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_page_names_map_to_their_encodings_case_insensitively() {
        assert_eq!(encoding_for("ANSI_949").map(|e| e.name()), Some("EUC-KR"));
        assert_eq!(
            encoding_for("ansi_1252").map(|e| e.name()),
            Some("windows-1252")
        );
        assert_eq!(
            encoding_for("ANSI_932").map(|e| e.name()),
            Some("Shift_JIS")
        );
        assert_eq!(encoding_for("ANSI_9999"), None);
        assert_eq!(encoding_for("UTF-8"), None);
    }

    #[test]
    fn valid_utf8_is_taken_as_is_whatever_the_header_says() {
        let text =
            "  0\nSECTION\n  2\nHEADER\n  9\n$DWGCODEPAGE\n  3\nANSI_949\n  0\nENDSEC\n  0\nEOF\n";
        let (out, warnings) = decode(text.as_bytes());
        assert_eq!(out, text);
        assert!(warnings.is_empty());
    }

    #[test]
    fn cp949_bytes_decode_under_a_declared_code_page_and_a_bad_byte_is_said() {
        let mut bytes = b"  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$DWGCODEPAGE\n  3\nANSI_949\n  0\nENDSEC\n  1\n".to_vec();
        bytes.extend_from_slice(b"\xB5\xB5\xB8\xE9\n"); // U+B3C4 U+BA74 in CP949
        let (out, warnings) = decode(&bytes);
        assert!(out.ends_with("\u{b3c4}\u{ba74}\n"), "{out:?}");
        assert!(warnings.is_empty());

        bytes.extend_from_slice(b"  1\n\xFF\xFF\n");
        let (out, warnings) = decode(&bytes);
        assert!(out.contains('\u{fffd}'));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("TEXT_ENCODING"));
    }

    #[test]
    fn the_other_names_a_code_page_goes_by_are_read() {
        for (name, encoding) in [
            ("GB2312", "GBK"),
            ("big5", "Big5"),
            ("CP932", "Shift_JIS"),
            ("CP949", "EUC-KR"),
            ("CP866", "IBM866"),
            ("ISO-8859-2", "ISO-8859-2"),
            ("MACINTOSH", "macintosh"),
            ("UTF8", "UTF-8"),
        ] {
            assert_eq!(
                encoding_for(name).map(|e| e.name()),
                Some(encoding),
                "{name}"
            );
        }
        assert_eq!(encoding_for("CP437"), None);
        assert_eq!(encoding_for("JOHAB"), None);
    }

    #[test]
    fn a_byte_the_code_page_has_no_character_for_is_said_not_passed_through() {
        // 0x81 is unassigned in windows-1252.
        let bytes = b"  9\n$DWGCODEPAGE\n  3\nANSI_1252\n  0\nENDSEC\n  1\nA\x81B\n";
        let (out, warnings) = decode(bytes);
        assert!(out.ends_with("A\u{fffd}B\n"), "{out:?}");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("TEXT_ENCODING"), "{}", warnings[0]);
    }

    #[test]
    fn a_pre_r2007_file_that_names_no_code_page_is_read_as_ansi_1252_and_says_so() {
        let bytes = b"  9\n$ACADVER\n  1\nAC1015\n  0\nENDSEC\n  1\n<> \xB5m\n";
        let (out, warnings) = decode(bytes);
        assert!(out.ends_with("<> \u{b5}m\n"), "{out:?}");
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].starts_with("CODEPAGE_ASSUMED"),
            "{}",
            warnings[0]
        );
    }

    #[test]
    fn an_unknown_code_page_is_reported_not_guessed() {
        let bytes = b"  9\n$DWGCODEPAGE\n  3\nANSI_9999\n  0\nENDSEC\n  1\n\xB5\xB5\n";
        let (_, warnings) = decode(bytes);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("CODEPAGE_UNSUPPORTED: ANSI_9999"));
    }
}
