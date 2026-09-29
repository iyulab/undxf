//! A 3DSOLID's or a REGION's ACIS body: the wireframe of the SAT text the
//! record holds, undone from how DXF stores it, or of the SAB bytes the
//! ACDSDATA section holds for it (R2013 on); an entity whose body is in
//! neither stays UNKNOWN.

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

/// SAB bytes of one straight edge from (0, 0, 0) to (1, 2, 3): the header,
/// then records 0 asmheader, 1-2 point, 3-4 vertex, 5 edge, then the
/// end-of-data marker.
fn sab_one_edge() -> Vec<u8> {
    let mut b = b"ASM BinaryFile4".to_vec();
    for v in [22300i32, 0, 2, 4] {
        b.extend(v.to_le_bytes());
    }
    let string = |b: &mut Vec<u8>, t: &str| {
        b.push(0x07);
        b.push(t.len() as u8);
        b.extend(t.as_bytes());
    };
    let ident = |b: &mut Vec<u8>, t: &str| {
        let parts: Vec<&str> = t.split('-').collect();
        for (i, part) in parts.iter().enumerate() {
            b.push(if i + 1 == parts.len() { 0x0D } else { 0x0E });
            b.push(part.len() as u8);
            b.extend(part.as_bytes());
        }
    };
    let ptr = |b: &mut Vec<u8>, v: i32| {
        b.push(0x0C);
        b.extend(v.to_le_bytes());
    };
    let int = |b: &mut Vec<u8>, v: i32| {
        b.push(0x04);
        b.extend(v.to_le_bytes());
    };
    for t in ["Product", "ASM 223.0", "Mon Jan 01 00:00:00 2024"] {
        string(&mut b, t);
    }
    for v in [1.0f64, 1e-6, 1e-10] {
        b.push(0x06);
        b.extend(v.to_le_bytes());
    }
    ident(&mut b, "asmheader");
    ptr(&mut b, -1);
    int(&mut b, -1);
    string(&mut b, "223.0.1.1930");
    b.push(0x11);
    for p in [[0.0f64, 0.0, 0.0], [1.0, 2.0, 3.0]] {
        ident(&mut b, "point");
        ptr(&mut b, -1);
        int(&mut b, -1);
        ptr(&mut b, -1);
        b.push(0x13);
        for v in p {
            b.extend(v.to_le_bytes());
        }
        b.push(0x11);
    }
    for point in [1, 2] {
        ident(&mut b, "vertex");
        for v in [-1, 5] {
            ptr(&mut b, v);
        }
        int(&mut b, 0);
        ptr(&mut b, point);
        b.push(0x11);
    }
    ident(&mut b, "edge");
    for v in [-1, 3, 4] {
        ptr(&mut b, v);
    }
    b.push(0x11);
    ident(&mut b, "End-of-ASM-data");
    b
}

/// An R2013-style drawing: the entity's record holds no body, and an
/// ACDSDATA record holds `sab` for handle 2E1 -- over two 310s, with `count`
/// in its 94 -- next to a thumbnail record the reader passes over.
fn acdsdata_drawing(type_name: &str, sab: &[u8], count: usize) -> String {
    let hex: String = sab.iter().map(|b| format!("{b:02X}")).collect();
    let (a, b) = hex.split_at(hex.len() / 2 / 2 * 2);
    format!(
        "  0
SECTION
  2
ENTITIES
  0
{type_name}
  5
2E1
100
AcDbEntity
  8
0
100
AcDbModelerGeometry
290
1
  2
{{7cf5f000-46fc-4a48-beef-14b47a01fb47}}
  0
ENDSEC
          0
SECTION
  2
ACDSDATA
 70
2
 71
2
          0
ACDSRECORD
 90
0
  2
AcDbDs::ID
280
10
320
22
  2
Thumbnail_Data
280
15
 94
2
310
ABCD
          0
ACDSRECORD
 90
1
  2
AcDbDs::ID
280
10
320
2E1
  2
ASM_Data
280
15
 94
{count}
310
{a}
310
{b}
          0
ENDSEC
  0
EOF
"
    )
}

#[test]
fn an_r2013_body_in_the_acdsdata_section_reads_as_its_wireframe() {
    let sab = sab_one_edge();
    for (type_name, want_region) in [("3DSOLID", false), ("REGION", true)] {
        let db = read_str(&acdsdata_drawing(type_name, &sab, sab.len())).unwrap();
        let (Entity::Solid3D(s) | Entity::Region(s)) = &db.entities[0] else {
            panic!("a solid, got {:?}", db.entities[0]);
        };
        assert_eq!(matches!(db.entities[0], Entity::Region(_)), want_region);
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
        assert!(
            db.read_diagnostics.warnings.is_empty(),
            "{:?}",
            db.read_diagnostics.warnings
        );
    }
}

#[test]
fn an_acdsdata_body_of_the_wrong_length_leaves_its_entity_unknown_and_says_so() {
    let sab = sab_one_edge();
    let db = read_str(&acdsdata_drawing("3DSOLID", &sab, sab.len() + 1)).unwrap();
    assert!(
        matches!(&db.entities[0], Entity::Unknown { type_name, .. } if type_name == "3DSOLID"),
        "{:?}",
        db.entities[0]
    );
    let warnings = &db.read_diagnostics.warnings;
    assert!(
        warnings.len() == 1 && warnings[0].starts_with("ACIS_BODY_UNREADABLE:"),
        "{warnings:?}"
    );
}

#[test]
fn an_acdsdata_body_that_does_not_decode_leaves_its_entity_unknown() {
    let mut sab = sab_one_edge();
    sab.truncate(sab.len() - 4);
    let db = read_str(&acdsdata_drawing("REGION", &sab, sab.len())).unwrap();
    assert!(
        matches!(&db.entities[0], Entity::Unknown { type_name, .. } if type_name == "REGION"),
        "{:?}",
        db.entities[0]
    );
}

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

/// The same drawing saved from R13 to R2018: every body -- in the record up
/// to R2010 (R13 and R14 with an ACIS 1.x header), in the ACDSDATA section
/// from R2013 -- reads to the same edge counts (REGION 4 · 3DSOLID 18 ·
/// REGION 4, none skipped).
#[test]
fn the_example_drawings_read_every_body() {
    if !Path::new(CORPUS).is_dir() {
        println!("skipped -- no corpus at {CORPUS}");
        return;
    }
    for version in ["r13", "r14", "2000", "2004", "2007", "2010", "2013", "2018"] {
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
