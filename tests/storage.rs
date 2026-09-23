//! How a file stores its strings -- escapes and caret notation -- is undone
//! as they are read; what the text itself says is not.

use uncad_model::model::{Entity, Ref};
use undxf::{read_bytes, read_str};

fn entities(section: &str) -> String {
    format!("  0\nSECTION\n  2\nENTITIES\n{section}  0\nENDSEC\n  0\nEOF\n")
}

#[test]
fn a_unicode_escape_reads_as_the_character_in_text_and_in_names() {
    let text = format!(
        "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n 70\n1\n  0\nLAYER\n  2\n\\U+0410\\U+043D\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n  0\nENDTAB\n  0\nENDSEC\n{}",
        entities("  0\nTEXT\n  5\n2A\n  8\n\\U+0410\\U+043D\n 10\n0\n 20\n0\n 40\n1\n  1\n108\\U+00B0 %%c32\n")
    );
    let db = read_str(&text).unwrap();
    let Entity::Text(t) = &db.entities[0] else {
        panic!("a TEXT, got {:?}", db.entities[0]);
    };
    // The escape is storage; the percent code is the text's own.
    assert_eq!(t.text, "108\u{b0} %%c32");
    // The layer's name and the entity's reference to it are undone alike,
    // so the reference still resolves.
    assert_eq!(t.common.layer, Ref::Resolved("\u{410}\u{43d}".to_string()));
    assert!(db.tables.layers.contains_key("\u{410}\u{43d}"));
}

#[test]
fn caret_notation_reads_as_the_control_character() {
    let db = read_str(&entities(
        "  0\nTOLERANCE\n  5\n2B\n  8\n0\n  3\nSTANDARD\n 10\n0\n 20\n0\n 30\n0\n  1\n{\\Fgdt;j}%%v0.1^J10\n",
    ))
    .unwrap();
    let Entity::Tolerance(t) = &db.entities[0] else {
        panic!("a TOLERANCE, got {:?}", db.entities[0]);
    };
    assert_eq!(t.text_value, "{\\Fgdt;j}%%v0.1\n10");
}

#[test]
fn an_escape_split_across_mtext_pieces_is_still_one_character() {
    // A long MTEXT is written in 250-character pieces (group 3), and a
    // piece may end inside an escape.
    let db = read_str(&entities(
        "  0\nMTEXT\n  5\n2C\n  8\n0\n 10\n0\n 20\n0\n 40\n1\n  3\nangle 90\\U+00\n  1\nB0\\Pnext\n",
    ))
    .unwrap();
    let Entity::MText(m) = &db.entities[0] else {
        panic!("an MTEXT, got {:?}", db.entities[0]);
    };
    // The paragraph break is MTEXT's own code: kept.
    assert_eq!(m.text, "angle 90\u{b0}\\Pnext");
}

#[test]
fn an_escape_of_an_ascii_character_is_left_as_written() {
    // Undoing `\U+005C` would make a backslash that starts an MTEXT code.
    let db = read_str(&entities(
        "  0\nMTEXT\n  5\n2D\n  8\n0\n 10\n0\n 20\n0\n 40\n1\n  1\n\\U+005CP\n",
    ))
    .unwrap();
    let Entity::MText(m) = &db.entities[0] else {
        panic!("an MTEXT, got {:?}", db.entities[0]);
    };
    assert_eq!(m.text, "\\U+005CP");
}

#[test]
fn a_multibyte_escape_goes_through_the_code_page_it_names() {
    // \M+3 is code page 949; B5B5 is U+B3C4 there.
    let db = read_bytes(
        entities("  0\nTEXT\n  5\n2E\n  8\n0\n 10\n0\n 20\n0\n 40\n1\n  1\n\\M+3B5B5\n").as_bytes(),
    )
    .unwrap();
    let Entity::Text(t) = &db.entities[0] else {
        panic!("a TEXT, got {:?}", db.entities[0]);
    };
    assert_eq!(t.text, "\u{b3c4}");
}
