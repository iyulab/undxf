//! Binary DXF: read as the ASCII file it stands for, so a drawing saved both
//! ways reads to the same model.

use std::path::Path;
use uncad_model::CadDatabase;

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

fn read(path: &str) -> CadDatabase {
    undxf::read_bytes(&std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Every number of `a` and `b` equal within `tol` relative, every other value
/// identical -- an ASCII file writes a real to 16 significant digits, the
/// binary one keeps all of it.
fn same(a: &serde_json::Value, b: &serde_json::Value, tol: f64, at: &str) {
    use serde_json::Value::*;
    // The two files state different values here: the ASCII file's
    // ARC_DIMENSION 399 writes its groups 16/26 as its 13/23 point and 17/27
    // as its 14/24 point, the binary file 16/26 as the 14/24 point and 17/27
    // as zero. Read as written, the models differ in exactly this point (in
    // the drawing's list and in the model space block that owns it).
    if at == "example_2000.entities[46].points.arc"
        || at == "example_2000.tables.block_records.*Model_Space.entities[45].points.arc"
    {
        return;
    }
    // The binary file is a save of its own: it states the ASCII file's
    // `$FINGERPRINTGUID` but another `$VERSIONGUID`.
    if at == "example_2000.header.versionguid" {
        assert_ne!(a, b, "{at}: the two saves state different identifiers");
        return;
    }
    match (a, b) {
        (Number(x), Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            assert!(
                (x - y).abs() <= tol * x.abs().max(y.abs()).max(1.0),
                "{at}: {x} != {y}"
            );
        }
        (Array(x), Array(y)) => {
            assert_eq!(x.len(), y.len(), "{at}: lengths");
            for (i, (x, y)) in x.iter().zip(y).enumerate() {
                same(x, y, tol, &format!("{at}[{i}]"));
            }
        }
        (Object(x), Object(y)) => {
            assert_eq!(
                x.keys().collect::<Vec<_>>(),
                y.keys().collect::<Vec<_>>(),
                "{at}: keys"
            );
            for (k, v) in x {
                same(v, &y[k], tol, &format!("{at}.{k}"));
            }
        }
        _ => assert_eq!(a, b, "{at}"),
    }
}

#[test]
fn a_drawing_saved_as_binary_reads_to_the_model_of_its_ascii_twin() {
    if !Path::new(CORPUS).is_dir() {
        println!("skipped -- no corpus at {CORPUS}");
        return;
    }
    let binary = read(&format!("{CORPUS}/example_2000.dxfb"));
    let ascii = read(&format!("{CORPUS}/example_2000.dxf"));
    assert_eq!(binary.entities.len(), 72);
    same(
        &serde_json::to_value(&binary).unwrap(),
        &serde_json::to_value(&ascii).unwrap(),
        1e-12,
        "example_2000",
    );
}

/// `example_2018.dxfb` is not the ASCII file's twin: its ENTITIES section
/// holds 40 records where `example_2018.dxf` holds 69. It reads, without a
/// diagnostic, to 42 top-level entities -- the count a second, independent
/// DXF reader gives for the same file.
#[test]
fn a_binary_file_reads_to_the_entities_it_holds() {
    if !Path::new(CORPUS).is_dir() {
        println!("skipped -- no corpus at {CORPUS}");
        return;
    }
    let db = read(&format!("{CORPUS}/example_2018.dxfb"));
    assert_eq!(db.entities.len(), 42);
    assert!(db.read_diagnostics.is_clean(), "{:?}", db.read_diagnostics);
}
