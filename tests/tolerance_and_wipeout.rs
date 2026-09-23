//! TOLERANCE and WIPEOUT, from their own groups.

use uncad_model::model::{Entity, Point2D, Point3D, Ref};
use undxf::read_str;

fn one(entity: &str, groups: &[(i32, &str)]) -> Entity {
    let mut text =
        format!("  0\nSECTION\n  2\nENTITIES\n  0\n{entity}\n  5\n2A\n100\nAcDbEntity\n  8\n0\n");
    for (code, value) in groups {
        text.push_str(&format!("{code:>3}\n{value}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let db = read_str(&text).unwrap();
    assert!(
        db.read_diagnostics.warnings.is_empty(),
        "{:?}",
        db.read_diagnostics.warnings
    );
    db.entities[0].clone()
}

#[test]
fn a_frame_not_turned_from_the_x_axis_writes_no_direction_and_has_the_default() {
    let Entity::Tolerance(t) = one(
        "TOLERANCE",
        &[
            (100, "AcDbFcf"),
            (3, "ISO-25"),
            (10, "5"),
            (20, "6"),
            (30, "0"),
            (1, r"{\Fgdt;r}%%v1"),
        ],
    ) else {
        panic!("a TOLERANCE");
    };
    assert_eq!(
        t.insertion_point,
        Point3D {
            x: 5.0,
            y: 6.0,
            z: 0.0
        }
    );
    assert_eq!(t.text_value, r"{\Fgdt;r}%%v1");
    assert_eq!(
        t.direction,
        Some(Point3D {
            x: 1.0,
            y: 0.0,
            z: 0.0
        })
    );
    assert_eq!(t.text_height, None);
    // No DIMSTYLE table in this file: the name is kept, unresolved.
    assert_eq!(t.style_name, Ref::Unresolved("ISO-25".to_string()));
}

#[test]
fn a_wipeout_is_its_clip_polygon_taken_from_pixels_and_closed_once() {
    // One pixel square of 10, inserted at (100, 200); the polygon's pixel
    // corners (-0.5, 0.5), (0.5, 0.5), (0.5, -0.5), written closed.
    let Entity::Wipeout(w) = one(
        "WIPEOUT",
        &[
            (100, "AcDbWipeout"),
            (10, "100"),
            (20, "200"),
            (30, "0"),
            (11, "10"),
            (21, "0"),
            (31, "0"),
            (12, "0"),
            (22, "10"),
            (32, "0"),
            (13, "1"),
            (23, "1"),
            (71, "2"),
            (91, "4"),
            (14, "-0.5"),
            (24, "0.5"),
            (14, "0.5"),
            (24, "0.5"),
            (14, "0.5"),
            (24, "-0.5"),
            (14, "-0.5"),
            (24, "0.5"),
        ],
    ) else {
        panic!("a WIPEOUT");
    };
    // Pixel rows run down the image: y 0.5 is the image's bottom edge.
    assert_eq!(
        w.boundary,
        [
            Point2D { x: 100.0, y: 200.0 },
            Point2D { x: 110.0, y: 200.0 },
            Point2D { x: 110.0, y: 210.0 },
        ]
    );
}
