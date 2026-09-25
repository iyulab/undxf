//! The DXF text as a sequence of (group code, value) pairs: every two lines
//! of an ASCII DXF file are one pair, the first a small integer and the
//! second its value.

use serde::{Deserialize, Serialize};

/// One group code and its value, with the line the code was read on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair<'a> {
    pub code: i32,
    pub value: &'a str,
    /// 1-based line of the code, for error messages.
    pub line: usize,
}

/// Why a file could not be read. Every variant names the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ReadError {
    /// A group code line that is not an integer.
    BadGroupCode { line: usize, text: String },
    /// The text ends after a group code, before its value.
    TruncatedPair { line: usize },
    /// A value that must be a number is not one.
    BadNumber {
        line: usize,
        code: i32,
        text: String,
    },
    /// The pairs do not form the sections and records a DXF file has.
    Structure { line: usize, detail: String },
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::BadGroupCode { line, text } => {
                write!(f, "line {line}: group code {text:?} is not an integer")
            }
            ReadError::TruncatedPair { line } => {
                write!(
                    f,
                    "line {line}: the text ends before the value of this group code"
                )
            }
            ReadError::BadNumber { line, code, text } => {
                write!(
                    f,
                    "line {line}: group {code} value {text:?} is not a number"
                )
            }
            ReadError::Structure { line, detail } => write!(f, "line {line}: {detail}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Every pair of the text, in order. A value keeps its bytes as written
/// except for the line ending; a group code line is trimmed.
pub fn pairs(text: &str) -> Result<Vec<Pair<'_>>, ReadError> {
    // A DOS-era file ends with a SUB (0x1A) end-of-file marker after its
    // last line; it is not a group code.
    let text = text.trim_end_matches(['\u{1a}', '\n', '\r']);
    let mut out = Vec::new();
    let mut lines = text.split('\n').enumerate().peekable();
    while let Some((i, code_line)) = lines.next() {
        let line = i + 1;
        let code_text = code_line.trim();
        if code_text.is_empty() && lines.peek().is_none() {
            break; // a trailing newline
        }
        let code: i32 = code_text.parse().map_err(|_| ReadError::BadGroupCode {
            line,
            text: code_text.to_string(),
        })?;
        let Some((_, value_line)) = lines.next() else {
            return Err(ReadError::TruncatedPair { line });
        };
        let value = value_line.strip_suffix('\r').unwrap_or(value_line);
        out.push(Pair { code, value, line });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_lines_are_one_pair_and_the_value_keeps_its_spaces() {
        let p = pairs("  0\nSECTION\n  1\n  hello \r\n").unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].code, p[0].value, p[0].line), (0, "SECTION", 1));
        assert_eq!((p[1].code, p[1].value, p[1].line), (1, "  hello ", 3));
    }

    #[test]
    fn a_code_that_is_not_an_integer_and_a_missing_value_are_errors_with_lines() {
        assert_eq!(
            pairs("  0\nSECTION\nabc\nx\n").unwrap_err(),
            ReadError::BadGroupCode {
                line: 3,
                text: "abc".into()
            }
        );
        assert_eq!(
            pairs("  0\nSECTION\n  2").unwrap_err(),
            ReadError::TruncatedPair { line: 3 }
        );
    }
}
