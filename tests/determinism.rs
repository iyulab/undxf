//! The same text must give the same drawing, byte for byte, every time.

use undxf::read_str;

/// With a handful of entries in a leaked hash set, two consecutive
/// identical orders are already unlikely; this many leave no realistic
/// chance of a false pass.
const RUNS: usize = 24;

const G1: &str = include_str!("golden/g1.dxf");

#[test]
fn repeated_reads_are_byte_identical() {
    let first = serde_json::to_string(&read_str(G1).unwrap()).unwrap();
    for run in 1..RUNS {
        let again = serde_json::to_string(&read_str(G1).unwrap()).unwrap();
        assert_eq!(first, again, "run {run}: the drawing changed");
    }
}

#[test]
fn a_read_never_reports_what_it_did_not_read() {
    // Nothing in G1 is outside what this crate interprets, so the read is
    // clean and every entity has a known type.
    let db = read_str(G1).unwrap();
    assert!(db.read_diagnostics.is_clean());
    assert!(db
        .all_entities()
        .all(|e| !matches!(e, uncad_model::model::Entity::Unknown { .. })));
}
