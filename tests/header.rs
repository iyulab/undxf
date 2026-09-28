//! The HEADER section beside the drawing: every variable the file states,
//! read in the same pass as the drawing.

use std::path::Path;
use uncad_model::{Point2D, Point3D};
use undxf::{read_bytes_with_header, read_str_with_header};

const TEXT: &str = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$INSUNITS\n 70\n     4\n  9\n$EXTMIN\n 10\n-1.5\n 20\n2\n 30\n0\n  9\n$DIMPOST\n  1\n\n  9\n$CLAYER\n  8\nWalls\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";

#[test]
fn the_header_is_every_variable_the_file_states() {
    let (_, header) = read_str_with_header(TEXT).unwrap();
    let names: Vec<&str> = header.variables.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        ["ACADVER", "CLAYER", "DIMPOST", "EXTMIN", "INSUNITS"]
    );
    assert_eq!(header.text("ACADVER").as_deref(), Some("AC1015"));
    assert_eq!(header.int("INSUNITS"), Some(4));
    assert_eq!(
        header.point3("EXTMIN"),
        Some(Point3D {
            x: -1.5,
            y: 2.0,
            z: 0.0
        })
    );
    // Stated empty is not unstated.
    assert_eq!(header.text("DIMPOST").as_deref(), Some(""));
    assert_eq!(header.text("DIMSCALE"), None);
    let clayer = header.groups("CLAYER").unwrap();
    assert_eq!((clayer[0].code, clayer[0].value.as_str()), (8, "Walls"));
}

#[test]
fn a_variable_written_twice_keeps_its_last_writing_and_says_so() {
    let text = TEXT.replace(
        "  9\n$CLAYER\n  8\nWalls\n",
        "  9\n$CLAYER\n  8\nWalls\n  9\n$CLAYER\n  8\nDoors\n",
    );
    let (db, header) = read_str_with_header(&text).unwrap();
    assert_eq!(header.text("CLAYER").as_deref(), Some("Doors"));
    assert!(
        db.read_diagnostics
            .warnings
            .iter()
            .any(|w| w.starts_with("HEADER_VARIABLE_REPEATED: $CLAYER")),
        "{:?}",
        db.read_diagnostics.warnings
    );
}

#[test]
fn a_file_without_a_header_section_states_nothing() {
    let (_, header) =
        read_str_with_header("  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n").unwrap();
    assert!(header.variables.is_empty());
}

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

#[test]
fn a_real_header_reads_with_its_values_as_written() {
    let path = format!("{CORPUS}/example_2000.dxf");
    if !Path::new(&path).is_file() {
        println!("skipped -- no corpus at {CORPUS}");
        return;
    }
    let (_, header) = read_bytes_with_header(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(header.text("DWGCODEPAGE").as_deref(), Some("ANSI_1252"));
    assert_eq!(header.int("INSUNITS"), Some(4));
    assert_eq!(
        header.point2("LIMMAX"),
        Some(Point2D { x: 420.0, y: 297.0 })
    );
    assert_eq!(header.text("CLAYER").as_deref(), Some("Tavolo 3"));
    assert_eq!(
        header.point3("EXTMIN").map(|p| p.z),
        Some(-215.9295327147615)
    );
}
