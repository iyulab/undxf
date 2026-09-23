//! MLINE, and the multiline styles it names from the OBJECTS section.

use uncad_model::model::{Entity, Point3D, Ref};
use undxf::read_str;

const DRAWING: &str = "  0\nSECTION\n  2\nENTITIES\n  0\nMLINE\n  5\n2C\n100\nAcDbEntity\n  8\n0\n100\nAcDbMline\n  2\nWALL\n340\n1F\n 40\n1\n 70\n0\n 71\n3\n 72\n2\n 73\n2\n 10\n0\n 20\n0\n 30\n0\n 11\n0\n 21\n0\n 31\n0\n 12\n1\n 22\n0\n 32\n0\n 13\n0\n 23\n1\n 33\n0\n 74\n2\n 41\n0\n 41\n0\n 75\n0\n 74\n2\n 41\n0\n 41\n0\n 75\n0\n 11\n10\n 21\n0\n 31\n0\n 12\n1\n 22\n0\n 32\n0\n 13\n0\n 23\n1\n 33\n0\n 74\n2\n 41\n0\n 41\n0\n 75\n0\n 74\n2\n 41\n0\n 41\n0\n 75\n0\n  0\nENDSEC\n  0\nSECTION\n  2\nOBJECTS\n  0\nMLINESTYLE\n  5\n1F\n100\nAcDbMlineStyle\n  2\nWALL\n 70\n0\n  3\n\n 62\n256\n 51\n90\n 52\n90\n 71\n2\n 49\n0.5\n 62\n256\n  6\nBYLAYER\n 49\n-0.5\n 62\n256\n  6\nBYLAYER\n  0\nENDSEC\n  0\nEOF\n";

#[test]
fn a_multiline_keeps_its_points_and_miters_and_names_a_declared_style() {
    let db = read_str(DRAWING).unwrap();
    assert!(
        db.read_diagnostics.warnings.is_empty(),
        "{:?}",
        db.read_diagnostics.warnings
    );
    let Entity::MLine(m) = &db.entities[0] else {
        panic!("an MLINE, got {:?}", db.entities[0]);
    };
    let p = |x| Point3D { x, y: 0.0, z: 0.0 };
    let up = Point3D {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    assert_eq!(m.vertices.len(), 2);
    assert_eq!(
        (m.vertices[0].point, m.vertices[0].miter_direction),
        (p(0.0), up)
    );
    assert_eq!(
        (m.vertices[1].point, m.vertices[1].miter_direction),
        (p(10.0), up)
    );
    // 71 = 3: bit 2 is set -- closed.
    assert!(m.closed);
    assert_eq!(m.mlinestyle_name, Ref::Resolved("WALL".to_string()));
    assert_eq!(db.tables.mlinestyles["WALL"], [0.5, -0.5]);
}

#[test]
fn a_style_the_objects_do_not_declare_stays_a_name() {
    let without_objects = DRAWING.replace(
        "  2\nOBJECTS\n  0\nMLINESTYLE",
        "  2\nOBJECTS\n  0\nDICTIONARY",
    );
    let db = read_str(&without_objects).unwrap();
    let Entity::MLine(m) = &db.entities[0] else {
        panic!("an MLINE");
    };
    assert_eq!(m.mlinestyle_name, Ref::Unresolved("WALL".to_string()));
    assert!(db.tables.mlinestyles.is_empty());
}
