//! ACAD_TABLE, read as the block reference it is.

use uncad_model::model::{Entity, Point3D, Ref};
use undxf::read_str;

fn table(direction: (&str, &str), block_defined: bool) -> Entity {
    let mut text = String::from("  0\nSECTION\n  2\nBLOCKS\n");
    if block_defined {
        text.push_str("  0\nBLOCK\n  8\n0\n  2\n*T1\n 70\n1\n 10\n0\n 20\n0\n 30\n0\n  3\n*T1\n  0\nENDBLK\n  8\n0\n");
    }
    text.push_str("  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n");
    text.push_str("  0\nACAD_TABLE\n  5\n2A\n100\nAcDbEntity\n  8\n0\n100\nAcDbBlockReference\n");
    text.push_str("  2\n*T1\n 10\n12\n 20\n34\n 30\n0\n100\nAcDbTable\n342\n29B\n343\n4F3\n");
    text.push_str(&format!(
        " 11\n{}\n 21\n{}\n 31\n0\n 90\n22\n 91\n2\n 92\n3\n",
        direction.0, direction.1
    ));
    // Row heights and column widths reuse codes of their own; none of them
    // is a scale or an insertion point.
    text.push_str("141\n7.5\n141\n7.5\n142\n40\n142\n40\n142\n40\n");
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let db = read_str(&text).unwrap();
    assert!(
        db.read_diagnostics.warnings.is_empty(),
        "{:?}",
        db.read_diagnostics.warnings
    );
    db.entities
        .into_iter()
        .find(|e| matches!(e, Entity::AcadTable(_)))
        .expect("a table")
}

#[test]
fn a_table_carries_its_block_its_insertion_point_and_a_unit_scale() {
    let Entity::AcadTable(t) = table(("1", "0"), true) else {
        unreachable!()
    };
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
    let Entity::AcadTable(t) = table(("0", "1"), true) else {
        unreachable!()
    };
    assert!((t.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
}

#[test]
fn a_table_naming_a_block_the_file_does_not_define_keeps_the_name() {
    let Entity::AcadTable(t) = table(("1", "0"), false) else {
        unreachable!()
    };
    assert_eq!(t.block_name, Ref::Unresolved("*T1".to_string()));
}
