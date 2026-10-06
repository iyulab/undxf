//! ACAD_TABLE: the block reference it is, and its grid of cells.

use uncad_model::model::{AcadTableEntity, Entity, Point3D, Ref, TableCellKind, TableFlow};
use undxf::read_str;

/// A cell as R2000 writes it: kind 1 (text), not covered, spanning
/// nothing, its text directly in group 1.
fn cell_r2000(text: &str) -> String {
    format!(
        "171\n1\n172\n0\n173\n0\n174\n0\n175\n1\n176\n1\n177\n0\n178\n0\n145\n0.0\n  1\n{text}\n"
    )
}

/// A cell as R2007 and later write it: its value in a `CELL_VALUE` list
/// that reuses 90 to 94 -- 91 and 92 being the table's own row and column
/// counts before the first cell.
fn cell_r2007(value: &str) -> String {
    format!(
        "171\n1\n172\n0\n173\n0\n174\n0\n175\n1\n176\n1\n 91\n262144\n178\n0\n145\n0.0\n 92\n0\n\
         301\nCELL_VALUE\n 93\n6\n{value}304\nACVALUE_END\n"
    )
}

/// A DXF with one ACAD_TABLE whose table part states `rows` and `columns`
/// with a height per row and a width per column, followed by `cells`.
fn drawing(
    direction: (&str, &str),
    block_defined: bool,
    rows: usize,
    columns: usize,
    cells: &str,
) -> String {
    let mut text = String::from("  0\nSECTION\n  2\nBLOCKS\n");
    if block_defined {
        text.push_str("  0\nBLOCK\n  8\n0\n  2\n*T1\n 70\n1\n 10\n0\n 20\n0\n 30\n0\n  3\n*T1\n  0\nENDBLK\n  8\n0\n");
    }
    text.push_str("  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n");
    text.push_str("  0\nACAD_TABLE\n  5\n2A\n100\nAcDbEntity\n  8\n0\n100\nAcDbBlockReference\n");
    text.push_str("  2\n*T1\n 10\n12\n 20\n34\n 30\n0\n100\nAcDbTable\n342\n29B\n343\n4F3\n");
    text.push_str(&format!(
        " 11\n{}\n 21\n{}\n 31\n0\n 90\n22\n 91\n{rows}\n 92\n{columns}\n",
        direction.0, direction.1
    ));
    // Row heights and column widths reuse codes of their own; none of them
    // is a scale or an insertion point.
    for _ in 0..rows {
        text.push_str("141\n7.5\n");
    }
    for _ in 0..columns {
        text.push_str("142\n40\n");
    }
    text.push_str(cells);
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    text
}

fn read(text: &str) -> (AcadTableEntity, Vec<String>) {
    let db = read_str(text).unwrap();
    let table = db
        .entities
        .into_iter()
        .find_map(|e| match e {
            Entity::AcadTable(t) => Some(t),
            _ => None,
        })
        .expect("a table");
    (table, db.read_diagnostics.warnings)
}

/// A 2 × 3 table of empty R2000 cells, read without a warning.
fn table(direction: (&str, &str), block_defined: bool) -> AcadTableEntity {
    let (t, warnings) = read(&drawing(
        direction,
        block_defined,
        2,
        3,
        &cell_r2000("").repeat(6),
    ));
    assert!(warnings.is_empty(), "{warnings:?}");
    t
}

#[test]
fn a_table_carries_its_block_its_insertion_point_and_a_unit_scale() {
    let t = table(("1", "0"), true);
    assert_eq!(t.block_name, Ref::Resolved("*T1".to_string()));
    assert_eq!(
        t.insertion_point,
        Point3D {
            x: 12.0,
            y: 34.0,
            z: 0.0
        }
    );
    assert_eq!(
        t.scale,
        Point3D {
            x: 1.0,
            y: 1.0,
            z: 1.0
        }
    );
    assert_eq!(t.rotation, 0.0);
}

#[test]
fn a_tables_rotation_is_the_angle_of_its_horizontal_direction() {
    let t = table(("0", "1"), true);
    assert!((t.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
}

#[test]
fn a_table_naming_a_block_the_file_does_not_define_keeps_the_name() {
    let t = table(("1", "0"), false);
    assert_eq!(t.block_name, Ref::Unresolved("*T1".to_string()));
}

#[test]
fn a_table_carries_its_grid_row_by_row() {
    let t = table(("1", "0"), true);
    let grid = t.grid.expect("a grid");
    assert_eq!(grid.column_widths, [40.0, 40.0, 40.0]);
    assert_eq!(grid.rows.len(), 2);
    for row in &grid.rows {
        assert_eq!(row.height, 7.5);
        assert_eq!(row.cells.len(), 3);
        for cell in &row.cells {
            assert_eq!(cell.kind, Some(TableCellKind::Text));
            assert_eq!(cell.text, None, "an empty cell says nothing");
            assert!(!cell.covered);
            assert_eq!((cell.span_columns, cell.span_rows), (1, 1));
        }
    }
}

#[test]
fn a_merged_cell_spans_and_the_cells_under_it_are_covered() {
    // The first row is one cell over two columns: the file writes the
    // merged cell's span and marks the cell it covers.
    let covered = "171\n1\n173\n1\n175\n1\n176\n1\n  1\n\n";
    let cells = format!(
        "171\n1\n173\n0\n175\n2\n176\n1\n  1\nPARTS LIST\n{covered}{}{}",
        cell_r2000("M6"),
        cell_r2000(r"{\fArial;S45C}")
    );
    let (t, warnings) = read(&drawing(("1", "0"), true, 2, 2, &cells));
    assert!(warnings.is_empty(), "{warnings:?}");
    let rows = t.grid.expect("a grid").rows;
    let header = &rows[0].cells;
    assert_eq!(header[0].text.as_deref(), Some("PARTS LIST"));
    assert_eq!((header[0].span_columns, header[0].span_rows), (2, 1));
    assert!(!header[0].covered);
    assert!(header[1].covered);
    // The text is the file's, format codes and all.
    assert_eq!(rows[1].cells[0].text.as_deref(), Some("M6"));
    assert_eq!(rows[1].cells[1].text.as_deref(), Some(r"{\fArial;S45C}"));
}

#[test]
fn a_cell_value_list_does_not_lend_its_groups_to_the_table() {
    // From R2007 a cell's value list repeats 91 and 92; only the ones
    // before the first cell count rows and columns.
    let string = |s: &str| format!(" 90\n4\n  1\n{s}\n 94\n0\n300\n\n302\n{s}\n");
    let nothing = " 90\n0\n 94\n0\n300\n\n302\n\n";
    let cells = [
        cell_r2007(&string("Qty")),
        cell_r2007(&string("")),
        cell_r2007(nothing),
        cell_r2007(&string("4")),
    ]
    .concat();
    let (t, warnings) = read(&drawing(("1", "0"), true, 2, 2, &cells));
    assert!(warnings.is_empty(), "{warnings:?}");
    let grid = t.grid.expect("a grid");
    assert_eq!((grid.rows.len(), grid.column_widths.len()), (2, 2));
    let texts: Vec<_> = grid
        .rows
        .iter()
        .flat_map(|r| r.cells.iter().map(|c| c.text.as_deref()))
        .collect();
    // An empty string and no value are the same empty cell.
    assert_eq!(texts, [Some("Qty"), None, None, Some("4")]);
}

#[test]
fn the_same_empty_cell_spelled_by_two_versions_reads_the_same() {
    let r2000 = read(&drawing(("1", "0"), true, 1, 1, &cell_r2000(""))).0;
    let r2007 = read(&drawing(
        ("1", "0"),
        true,
        1,
        1,
        &cell_r2007(" 90\n0\n 94\n0\n"),
    ))
    .0;
    assert_eq!(r2000.grid, r2007.grid);
}

#[test]
fn a_block_cell_has_no_text() {
    let cells = "171\n2\n173\n0\n175\n1\n176\n1\n340\n1F\n";
    let (t, warnings) = read(&drawing(("1", "0"), true, 1, 1, cells));
    assert!(warnings.is_empty(), "{warnings:?}");
    let cell = &t.grid.expect("a grid").rows[0].cells[0];
    assert_eq!(
        (cell.kind, cell.text.as_deref()),
        (Some(TableCellKind::Block), None)
    );
}

#[test]
fn a_cell_kind_the_format_does_not_define_is_none() {
    let cells = "171\n9\n  1\nx\n";
    let (t, _) = read(&drawing(("1", "0"), true, 1, 1, cells));
    let cell = &t.grid.expect("a grid").rows[0].cells[0];
    assert_eq!((cell.kind, cell.text.as_deref()), (None, Some("x")));
}

#[test]
fn counts_that_do_not_make_a_grid_are_reported_and_leave_no_grid() {
    // Two by two needs four cells; three would shift every cell after the
    // missing one into the wrong place.
    let (t, warnings) = read(&drawing(("1", "0"), true, 2, 2, &cell_r2000("a").repeat(3)));
    assert_eq!(t.grid, None);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("TABLE_GRID:"), "{warnings:?}");
}

#[test]
fn a_span_that_is_not_a_count_is_reported_and_leaves_no_grid() {
    let cells = "171\n1\n175\n-1\n  1\na\n";
    let (t, warnings) = read(&drawing(("1", "0"), true, 1, 1, cells));
    assert_eq!(t.grid, None);
    assert!(
        warnings.iter().any(|w| w.starts_with("TABLE_GRID:")),
        "{warnings:?}"
    );
}

#[test]
fn a_table_stating_no_counts_has_no_grid_and_nothing_to_report() {
    let text = drawing(("1", "0"), true, 0, 0, "").replace(" 91\n0\n 92\n0\n", "");
    let (t, warnings) = read(&text);
    assert_eq!(t.grid, None);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_table_with_no_rows_is_an_empty_grid_not_an_unread_one() {
    let (t, warnings) = read(&drawing(("1", "0"), true, 0, 2, ""));
    assert!(warnings.is_empty(), "{warnings:?}");
    let grid = t.grid.expect("a grid");
    assert!(grid.rows.is_empty());
    assert_eq!(grid.column_widths.len(), 2);
}

#[test]
fn a_text_continued_over_several_groups_is_reported_and_left_unread() {
    // The joining rule is not exercised by any drawing at hand, so the cell
    // says it was not read rather than giving a cut-off text.
    let cells = "171\n1\n175\n1\n176\n1\n  2\nfirst part \n  1\nlast part\n";
    let (t, warnings) = read(&drawing(("1", "0"), true, 1, 1, cells));
    let cell = &t.grid.expect("the grid stands").rows[0].cells[0];
    assert_eq!(cell.text, None);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("TABLE_CELL_TEXT:"), "{warnings:?}");
}

#[test]
fn a_value_that_is_not_a_string_is_reported_and_left_unread() {
    // 90 = 2: a real number, shown through a format this reader does not apply.
    let (t, warnings) = read(&drawing(
        ("1", "0"),
        true,
        1,
        1,
        &cell_r2007(" 90\n2\n140\n4.0\n 94\n0\n"),
    ));
    assert_eq!(t.grid.expect("the grid stands").rows[0].cells[0].text, None);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("TABLE_CELL_TEXT:"), "{warnings:?}");
}

/// The 2 × 3 table with its own flow direction (group 70) when `own` is
/// given, and an OBJECTS section holding the TABLESTYLE it points at
/// (handle 29B) with flow `style` when that is given.
fn flowing(own: Option<&str>, style: Option<&str>) -> (AcadTableEntity, Vec<String>) {
    let mut text = drawing(("1", "0"), true, 2, 3, &cell_r2000("").repeat(6));
    if let Some(own) = own {
        text = text.replacen(" 92\n3\n", &format!(" 92\n3\n 93\n0\n 70\n{own}\n"), 1);
    }
    if let Some(style) = style {
        text = text.replacen(
            "  0\nEOF\n",
            &format!(
                "  0\nSECTION\n  2\nOBJECTS\n  0\nTABLESTYLE\n  5\n29B\n100\nAcDbTableStyle\n  3\nStandard\n 70\n{style}\n 71\n0\n  0\nENDSEC\n  0\nEOF\n"
            ),
            1,
        );
    }
    read(&text)
}

#[test]
fn a_table_that_states_no_flow_takes_its_styles() {
    let (t, warnings) = flowing(None, Some("1"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(t.flow, Some(TableFlow::Up));
    assert_eq!(flowing(None, Some("0")).0.flow, Some(TableFlow::Down));
}

#[test]
fn a_tables_own_flow_overrides_its_styles() {
    let (t, warnings) = flowing(Some("0"), Some("1"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(t.flow, Some(TableFlow::Down));
}

#[test]
fn a_flow_nobody_states_is_not_known() {
    // The style it points at is not in the drawing.
    assert_eq!(flowing(None, None).0.flow, None);
    // The style states a code the format does not define.
    assert_eq!(flowing(None, Some("7")).0.flow, None);
}

#[test]
fn a_flow_code_the_format_does_not_define_is_not_read_and_said_so() {
    let (t, warnings) = flowing(Some("5"), Some("0"));
    assert_eq!(t.flow, None);
    assert!(
        warnings.iter().any(|w| w.starts_with("TABLE_FLOW")),
        "{warnings:?}"
    );
}
