//! An INSERT that places its block as an array (MINSERT): the model holds one
//! placement, so the reader says the rest are not read rather than dropping
//! them in silence; a plain INSERT, columns and rows of 1 or unstated, says
//! nothing.

use uncad_model::model::Entity;
use undxf::read_str;

fn insert(groups: &[(i32, &str)]) -> String {
    let mut text = String::from(
        "  0\nSECTION\n  2\nENTITIES\n  0\nINSERT\n  5\n2A\n  8\n0\n  2\nHOLE\n 10\n0\n 20\n0\n 30\n0\n",
    );
    for (code, value) in groups {
        text.push_str(&format!("{code:>3}\n{value}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    text
}

#[test]
fn an_array_of_placements_is_read_as_its_first_and_reported() {
    let db = read_str(&insert(&[(70, "4"), (71, "2"), (44, "10"), (45, "20")])).unwrap();
    assert!(matches!(db.entities[..], [Entity::Insert(_)]));
    let warnings = &db.read_diagnostics.warnings;
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("MINSERT:") && w.contains("4 columns and 2 rows")),
        "{warnings:?}"
    );
}

#[test]
fn a_single_placement_is_not_reported() {
    for groups in [&[][..], &[(70, "1"), (71, "1")][..]] {
        let db = read_str(&insert(groups)).unwrap();
        assert!(
            !db.read_diagnostics
                .warnings
                .iter()
                .any(|w| w.starts_with("MINSERT:")),
            "{:?}",
            db.read_diagnostics.warnings
        );
    }
}
