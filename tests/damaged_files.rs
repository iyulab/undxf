//! Files that are damaged in a way that costs the drawing nothing: text after
//! the end-of-file pair, and a handle group written without a value. Both are
//! read, and both are reported. A file whose section structure is broken is
//! still refused.

use uncad_model::model::{Entity, Ref};
use uncad_model::CadDatabase;
use undxf::read_str;

const LINE: &str = "  0\nLINE\n  5\n2A\n  8\n0\n 10\n0\n 20\n0\n 11\n3\n 21\n4\n";

fn drawing(entity: &str, after_eof: &str) -> String {
    format!("  0\nSECTION\n  2\nENTITIES\n{entity}  0\nENDSEC\n  0\nEOF\n{after_eof}")
}

fn warnings(db: &CadDatabase) -> &[String] {
    &db.read_diagnostics.warnings
}

#[test]
fn text_after_the_eof_pair_is_not_read_and_is_reported() {
    let db = read_str(&drawing(LINE, "ignore\ninvalid\ndata\nbeyond\nEOF")).unwrap();
    assert_eq!(db.entities.len(), 1);
    assert!(
        warnings(&db)
            .iter()
            .any(|w| w.starts_with("TEXT_AFTER_EOF: 5 line(s) after the 0/EOF on line 21")),
        "{:?}",
        warnings(&db)
    );
}

#[test]
fn a_comment_after_the_eof_pair_is_not_text_the_format_has_no_place_for() {
    let db = read_str(&drawing(LINE, "999\nwritten by a tool\n")).unwrap();
    assert_eq!(db.entities.len(), 1);
    assert!(
        !warnings(&db)
            .iter()
            .any(|w| w.starts_with("TEXT_AFTER_EOF")),
        "{:?}",
        warnings(&db)
    );
}

#[test]
fn blank_lines_after_the_eof_pair_say_nothing() {
    let db = read_str(&drawing(LINE, "\n\r\n  \n")).unwrap();
    assert_eq!(db.entities.len(), 1);
    assert!(
        !warnings(&db)
            .iter()
            .any(|w| w.starts_with("TEXT_AFTER_EOF")),
        "{:?}",
        warnings(&db)
    );
}

/// Text after the pair that would be a valid section is still not read: the
/// format ends at the first 0/EOF.
#[test]
fn a_second_drawing_after_the_eof_pair_is_not_merged_in() {
    let second = drawing(LINE.replace("2A", "2B").as_str(), "");
    let db = read_str(&drawing(LINE, &second)).unwrap();
    assert_eq!(db.entities.len(), 1);
}

#[test]
fn an_empty_handle_is_an_entity_with_no_handle_and_says_so() {
    let line = LINE.replace("  5\n2A\n", "  5\n\n");
    let db = read_str(&drawing(&line, "")).unwrap();
    let Some(e) = db.entities.first() else {
        panic!("a LINE");
    };
    assert!(matches!(e, Entity::Line(_)));
    assert_eq!(e.common().source_handle, Ref::Absent);
    assert!(e.common().id.value() >= 1 << 63, "a handle-less ID");
    assert!(
        warnings(&db)
            .iter()
            .any(|w| w.starts_with("EMPTY_HANDLE: the handle group (5) of this LINE on line 7")),
        "{:?}",
        warnings(&db)
    );
}

/// Only an empty value is "no handle". Text that is not a number is still a
/// value the file states wrongly.
#[test]
fn a_handle_that_is_not_hexadecimal_is_still_refused() {
    let line = LINE.replace("  5\n2A\n", "  5\nZZ\n");
    let err = read_str(&drawing(&line, "")).unwrap_err();
    assert!(
        err.to_string()
            .contains("group 5 value \"ZZ\" is not a number"),
        "{err}"
    );
}

/// A broken section structure is not repaired: guessing which part is right
/// would be inferring the drawing.
#[test]
fn a_stray_endsec_at_the_top_level_is_still_refused() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nENDSEC\n  0\nEOF\n";
    let err = read_str(text).unwrap_err();
    assert!(
        err.to_string().contains("expected 0/SECTION or 0/EOF"),
        "{err}"
    );
}
