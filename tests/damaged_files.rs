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

// ---- what an independent read of the two rules found ------------------------

fn with_tables(tables: &str, entity: &str) -> String {
    format!(
        "  0\nSECTION\n  2\nTABLES\n{tables}  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n{entity}  0\nENDSEC\n  0\nEOF\n"
    )
}

/// A record that writes its handle empty has no handle to be found by: a
/// reference that writes nothing points at nothing, and does not land on it.
#[test]
fn an_empty_reference_does_not_find_a_record_whose_handle_is_empty() {
    let layers =
        "  0\nTABLE\n  2\nLAYER\n  0\nLAYER\n  5\n\n  2\nWALLS\n 70\n0\n 62\n7\n  0\nENDTAB\n";
    let viewport = "  0\nVIEWPORT\n  5\n2A\n 67\n1\n  8\n0\n 10\n100.0\n 20\n50.0\n 40\n200.0\n 41\n100.0\n 341\n\n 331\n   \n 341\n99\n";
    let db = read_str(&with_tables(layers, viewport)).unwrap();
    let Some(Entity::Viewport(v)) = db.entities.first() else {
        panic!("a viewport");
    };
    // The two that write nothing are not references at all.
    assert_eq!(v.frozen_layers, [Ref::Unresolved("99".to_string())]);
}

/// The warning about a handle group is one line for the file, not one for each
/// entity: a file with a thousand empty handles says so once.
#[test]
fn empty_handles_are_said_once_with_how_many() {
    let one = LINE.replace("  5\n2A\n", "  5\n\n");
    let many = one.repeat(1000);
    let db = read_str(&drawing(&many, "")).unwrap();
    assert_eq!(db.entities.len(), 1000);
    let said: Vec<&String> = warnings(&db)
        .iter()
        .filter(|w| w.starts_with("EMPTY_HANDLE"))
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("on line 7"), "{}", said[0]);
    assert!(said[0].ends_with("(999 more like it)"), "{}", said[0]);
}

/// A POLYLINE is read apart from the other entities; its handle warning still
/// reaches the file.
#[test]
fn an_empty_handle_on_a_polyline_is_said_too() {
    let polyline = "  0\nPOLYLINE\n  5\n\n  8\n0\n 66\n1\n 70\n0\n  0\nVERTEX\n  8\n0\n 10\n0\n 20\n0\n  0\nVERTEX\n  8\n0\n 10\n1\n 20\n0\n  0\nSEQEND\n";
    let db = read_str(&drawing(polyline, "")).unwrap();
    assert!(matches!(db.entities.first(), Some(Entity::Polyline2D(_))));
    assert!(
        warnings(&db)
            .iter()
            .any(|w| w.starts_with("EMPTY_HANDLE: the handle group (5) of this POLYLINE")),
        "{:?}",
        warnings(&db)
    );
}

/// A handle that is not below 2^63 would be the ID of a handle-less entity: it
/// is read as no handle, and said.
#[test]
fn a_handle_that_would_be_a_handle_less_id_is_not_used_as_one() {
    let big = LINE.replace("  5\n2A\n", "  5\n8000000000000001\n");
    let none = LINE.replace("  5\n2A\n", "");
    let db = read_str(&drawing(&format!("{big}{none}"), "")).unwrap();
    let ids: Vec<u64> = db.entities.iter().map(|e| e.common().id.value()).collect();
    assert_ne!(ids[0], ids[1], "{ids:?}");
    assert_eq!(db.entities[0].common().source_handle, Ref::Absent);
    assert!(
        warnings(&db)
            .iter()
            .any(|w| w.starts_with("HANDLE_OUT_OF_RANGE: the handle group (5) of this LINE")),
        "{:?}",
        warnings(&db)
    );
    // The largest handle below the bound is still a handle.
    let top = LINE.replace("  5\n2A\n", "  5\n7FFFFFFFFFFFFFFF\n");
    let db = read_str(&drawing(&top, "")).unwrap();
    assert_eq!(db.entities[0].common().id.value(), i64::MAX as u64);
}

/// The first handle group is the handle, for what is read from it and for what
/// is said about it.
#[test]
fn the_first_handle_group_decides_when_an_entity_writes_two() {
    let first_good = LINE.replace("  5\n2A\n", "  5\n2A\n  5\n\n");
    let db = read_str(&drawing(&first_good, "")).unwrap();
    assert_eq!(db.entities[0].common().id.value(), 0x2A);
    assert!(
        !warnings(&db).iter().any(|w| w.starts_with("EMPTY_HANDLE")),
        "{:?}",
        warnings(&db)
    );
    let first_empty = LINE.replace("  5\n2A\n", "  5\n\n  5\n2A\n");
    let db = read_str(&drawing(&first_empty, "")).unwrap();
    assert_eq!(db.entities[0].common().source_handle, Ref::Absent);
    assert!(
        warnings(&db).iter().any(|w| w.starts_with("EMPTY_HANDLE")),
        "{:?}",
        warnings(&db)
    );
}

/// NUL bytes written after the end-of-file pair are padding, like the DOS SUB.
#[test]
fn nul_padding_after_the_eof_pair_is_not_text() {
    let db = read_str(&format!("{}\0\0\0", drawing(LINE, "")).replace("EOF\n\0", "EOF\0")).unwrap();
    assert_eq!(db.entities.len(), 1);
    assert!(
        !warnings(&db)
            .iter()
            .any(|w| w.starts_with("TEXT_AFTER_EOF")),
        "{:?}",
        warnings(&db)
    );
    let db = read_str(&format!("{}\n\0\0", drawing(LINE, "").trim_end())).unwrap();
    assert_eq!(db.entities.len(), 1);
}
