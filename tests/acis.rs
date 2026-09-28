//! A 3DSOLID's or a REGION's ACIS body: the wireframe of the SAT text the
//! record holds, undone from how DXF stores it; a record whose body is not in
//! the record stays UNKNOWN.

use std::path::Path;
use uncad_model::model::{Entity, Point3D};
use undxf::read_str;

/// The DXF obfuscation, forward -- the inverse of what the reader undoes.
fn encode(line: &str) -> String {
    let mut out = String::new();
    for c in line.chars() {
        match c {
            ' ' => out.push(' '),
            '_' => out.push('@'),
            '@' => out.push('_'),
            'A'..='^' => {
                out.push(char::from_u32(0x5E - (c as u32 - 0x41)).unwrap());
                if c == 'A' {
                    out.push(' ');
                }
            }
            _ => out.push(char::from_u32(c as u32 ^ 0x5F).unwrap()),
        }
    }
    out
}

fn record(type_name: &str, sat: &[&str]) -> String {
    let mut text = format!(
        "  0\nSECTION\n  2\nENTITIES\n  0\n{type_name}\n  5\n2E1\n100\nAcDbEntity\n  8\n0\n100\nAcDbModelerGeometry\n 70\n1\n"
    );
    for line in sat {
        let enc = encode(line);
        // A long line continues in group 3s; split every body line once to
        // exercise that.
        let (a, b) = enc.split_at(enc.len() / 2);
        text.push_str(&format!("  1\n{a}\n  3\n{b}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    text
}

const BODY: &[&str] = &[
    "400 5 1 0",
    "8 Autodesk 11 ACIS 4.0 NT 24 Mon Jan 01 00:00:00 2024",
    "1 9.9999999999999995e-07 1e-10",
    "point $-1 0 0 0 #",
    "point $-1 1 2 3 #",
    "vertex $-1 $0 #",
    "vertex $-1 $1 #",
    "edge $-1 $2 $3 $-1 #",
    "End-of-ACIS-data",
];

#[test]
fn a_3dsolid_whose_record_holds_its_body_reads_as_its_wireframe() {
    let db = read_str(&record("3DSOLID", BODY)).unwrap();
    let Entity::Solid3D(s) = &db.entities[0] else {
        panic!("a 3DSOLID, got {:?}", db.entities[0]);
    };
    assert_eq!(s.skipped_edges, 0);
    assert_eq!(
        s.wireframe_edges,
        vec![[
            Point3D {
                x: 0.0,
                y: 0.0,
                z: 0.0
            },
            Point3D {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        ]]
    );
}

#[test]
fn a_region_reads_the_same_way_as_its_own_type() {
    let db = read_str(&record("REGION", BODY)).unwrap();
    assert!(
        matches!(&db.entities[0], Entity::Region(s) if s.wireframe_edges.len() == 1),
        "{:?}",
        db.entities[0]
    );
}

#[test]
fn a_body_stored_outside_the_record_stays_unknown() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\n3DSOLID\n  5\n2E1\n100\nAcDbEntity\n  8\n0\n100\nAcDbModelerGeometry\n290\n1\n  2\n{7cf5f000-46fc-4a48-beef-14b47a01fb47}\n100\nAcDb3dSolid\n350\n2E4\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    assert!(
        matches!(&db.entities[0], Entity::Unknown { type_name, .. } if type_name == "3DSOLID"),
        "{:?}",
        db.entities[0]
    );
}

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

/// The same drawing saved from R2000 to R2010: every body is in the record,
/// and each top-level one reads to the edge counts its DWG twin gives through a DWG reader
/// (REGION 4 · 3DSOLID 18 · REGION 4, none skipped).
#[test]
fn the_example_drawings_read_every_body_in_the_record() {
    if !Path::new(CORPUS).is_dir() {
        println!("skipped -- no corpus at {CORPUS}");
        return;
    }
    for version in ["2000", "2004", "2007", "2010"] {
        let path = format!("{CORPUS}/example_{version}.dxf");
        let db = undxf::read_bytes(&std::fs::read(&path).unwrap()).unwrap();
        let bodies: Vec<(&str, usize, usize)> = db
            .entities
            .iter()
            .filter_map(|e| match e {
                Entity::Solid3D(s) => Some(("3DSOLID", s.wireframe_edges.len(), s.skipped_edges)),
                Entity::Region(s) => Some(("REGION", s.wireframe_edges.len(), s.skipped_edges)),
                _ => None,
            })
            .collect();
        let mut sorted = bodies.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![("3DSOLID", 18, 0), ("REGION", 4, 0), ("REGION", 4, 0)],
            "example_{version}.dxf"
        );
    }
}
