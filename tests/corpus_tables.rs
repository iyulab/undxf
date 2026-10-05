//! The tables of the LibreDWG test corpus, when it is present beside this
//! crate (it is a GPL project's data and is not copied here): each one's
//! grid as its file states it, counted from the file's own groups.
//!
//! The same table saved by six versions spells an empty cell two ways --
//! an empty group 1 up to R2004, a value list with no value from R2007 --
//! and must read as one grid from all six.

use std::path::Path;
use uncad_model::model::{Entity, TableGrid};

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

/// The grids of the top-level tables of `name`, or `None` when the corpus
/// is not here.
fn grids(name: &str) -> Option<Vec<TableGrid>> {
    let path = Path::new(CORPUS).join(name);
    if !path.is_file() {
        println!("corpus file {} not present; skipped", path.display());
        return None;
    }
    let db = undxf::read_bytes(&std::fs::read(&path).unwrap()).unwrap();
    Some(
        db.entities
            .into_iter()
            .filter_map(|e| match e {
                Entity::AcadTable(t) => {
                    Some(t.grid.expect("every corpus table states a whole grid"))
                }
                _ => None,
            })
            .collect(),
    )
}

/// (rows, columns, cells with text, the texts in reading order).
fn shape(grid: &TableGrid) -> (usize, usize, Vec<&str>) {
    let texts = grid
        .rows
        .iter()
        .flat_map(|r| r.cells.iter().filter_map(|c| c.text.as_deref()))
        .collect();
    (grid.rows.len(), grid.column_widths.len(), texts)
}

#[test]
fn the_small_table_reads_the_same_from_r2000_and_r2018() {
    for name in ["2000/TS1.dxf", "2018/TS1.dxf"] {
        let Some(grids) = grids(name) else { return };
        assert_eq!(grids.len(), 1, "{name}");
        let grid = &grids[0];
        assert_eq!(
            shape(grid),
            (3, 5, vec!["some text", "some texst"]),
            "{name}"
        );
        // The title row is one cell over all five columns.
        let title = &grid.rows[0].cells;
        assert_eq!(
            (title[0].span_columns, title[0].span_rows),
            (5, 1),
            "{name}"
        );
        assert!(title[1..].iter().all(|c| c.covered), "{name}");
        assert_eq!(grid.rows[0].height, 0.4533333333333334, "{name}");
        assert_eq!(grid.column_widths, [2.5; 5], "{name}");
    }
}

#[test]
fn the_large_table_reads_as_one_grid_from_six_versions() {
    let mut first: Option<(String, TableGrid)> = None;
    for version in ["2000", "2004", "2007", "2010", "2013", "2018"] {
        let name = format!("example_{version}.dxf");
        let Some(grids) = grids(&name) else { return };
        assert_eq!(grids.len(), 1, "{name}");
        let grid = grids.into_iter().next().unwrap();
        let (rows, columns, texts) = shape(&grid);
        assert_eq!(
            (rows, columns, texts.len()),
            (20, 9, 7),
            "{name}: {texts:?}"
        );
        match &first {
            None => first = Some((name, grid)),
            Some((first_name, first_grid)) => {
                assert_eq!(&grid, first_grid, "{name} against {first_name}")
            }
        }
    }
}
