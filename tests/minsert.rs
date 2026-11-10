//! An INSERT that places its block as an array (MINSERT): the model carries
//! the grid -- columns, rows, and the distances between them -- so no
//! placement is lost; a plain INSERT, columns and rows of 1 or unstated, is
//! a single placement and carries no grid; a count that is no grid is
//! reported and read as a single placement.

use uncad_model::model::{Entity, InsertArray, InsertEntity};
use undxf::read_str;

fn insert(groups: &[(i32, &str)]) -> String {
    let mut text = String::from(
        "  0
SECTION
  2
ENTITIES
  0
INSERT
  5
2A
  8
0
  2
HOLE
 10
0
 20
0
 30
0
",
    );
    for (code, value) in groups {
        text.push_str(&format!(
            "{code:>3}
{value}
"
        ));
    }
    text.push_str(
        "  0
ENDSEC
  0
EOF
",
    );
    text
}

fn only_insert(text: &str) -> (InsertEntity, Vec<String>) {
    let db = read_str(text).unwrap();
    let warnings = db.read_diagnostics.warnings.clone();
    match &db.entities[..] {
        [Entity::Insert(i)] => (i.clone(), warnings),
        other => panic!("one INSERT expected: {other:?}"),
    }
}

#[test]
fn an_array_of_placements_is_read_with_its_grid() {
    let (i, warnings) = only_insert(&insert(&[(70, "4"), (71, "2"), (44, "10"), (45, "20.5")]));
    assert_eq!(
        i.array,
        Some(InsertArray {
            columns: 4,
            rows: 2,
            column_spacing: 10.0,
            row_spacing: 20.5,
        })
    );
    assert!(
        warnings.iter().all(|w| !w.starts_with("MINSERT:")),
        "{warnings:?}"
    );
}

#[test]
fn a_grid_in_one_direction_states_the_other_as_one_and_unstated_spacings_as_zero() {
    let (i, _) = only_insert(&insert(&[(70, "3"), (44, "5")]));
    assert_eq!(
        i.array,
        Some(InsertArray {
            columns: 3,
            rows: 1,
            column_spacing: 5.0,
            row_spacing: 0.0,
        })
    );
}

#[test]
fn a_single_placement_has_no_grid_and_is_not_reported() {
    for groups in [
        &[][..],
        &[(70, "1"), (71, "1")][..],
        &[(70, "1"), (71, "1"), (44, "7")][..],
    ] {
        let (i, warnings) = only_insert(&insert(groups));
        assert_eq!(i.array, None);
        assert!(
            warnings.iter().all(|w| !w.starts_with("MINSERT:")),
            "{warnings:?}"
        );
    }
}

#[test]
fn a_count_that_is_no_grid_is_reported_and_read_as_one_placement() {
    for groups in [
        &[(70, "0")][..],
        &[(70, "-2"), (71, "3")][..],
        &[(71, "40000")][..],
    ] {
        let (i, warnings) = only_insert(&insert(groups));
        assert_eq!(i.array, None);
        assert!(
            warnings
                .iter()
                .any(|w| w.starts_with("MINSERT:") && w.contains("not a grid")),
            "{warnings:?}"
        );
    }
}
