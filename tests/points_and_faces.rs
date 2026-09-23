//! POINT, SOLID, TRACE and 3DFACE: their positions and corners, and what a
//! missing fourth corner or position means.

use uncad_model::model::Entity;
use undxf::read_str;

/// A DXF with one entity of `type_name` carrying `groups`, in order.
fn one(type_name: &str, groups: &[(i32, &str)]) -> String {
    let mut text = format!("  0\nSECTION\n  2\nENTITIES\n  0\n{type_name}\n  5\n2A\n  8\n0\n");
    for (code, value) in groups {
        text.push_str(&format!("{code:>3}\n{value}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    text
}

#[test]
fn a_point_reads_its_position() {
    let db = read_str(&one("POINT", &[(10, "1"), (20, "2"), (30, "3")])).unwrap();
    let Entity::Point(p) = &db.entities[0] else {
        panic!("a POINT, got {:?}", db.entities[0]);
    };
    assert_eq!((p.position.x, p.position.y, p.position.z), (1.0, 2.0, 3.0));
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn a_solid_without_a_fourth_corner_has_its_third_there() {
    let text = one(
        "SOLID",
        &[
            (10, "0"),
            (20, "0"),
            (11, "4"),
            (21, "0"),
            (12, "0"),
            (22, "3"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Solid(s) = &db.entities[0] else {
        panic!("a SOLID, got {:?}", db.entities[0]);
    };
    assert_eq!((s.corner2.x, s.corner3.y), (4.0, 3.0));
    assert_eq!(s.corner4, s.corner3);
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn a_trace_is_read_as_a_trace_with_its_four_corners() {
    let text = one(
        "TRACE",
        &[
            (10, "0"),
            (20, "0"),
            (11, "1"),
            (21, "0"),
            (12, "0"),
            (22, "1"),
            (13, "1"),
            (23, "1"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Trace(t) = &db.entities[0] else {
        panic!("a TRACE, got {:?}", db.entities[0]);
    };
    assert_eq!((t.corner4.x, t.corner4.y), (1.0, 1.0));
}

#[test]
fn a_3dface_reads_its_corners_in_three_dimensions() {
    let text = one(
        "3DFACE",
        &[
            (10, "0"),
            (20, "0"),
            (30, "1"),
            (11, "1"),
            (21, "0"),
            (31, "2"),
            (12, "1"),
            (22, "1"),
            (32, "3"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Face3D(f) = &db.entities[0] else {
        panic!("a 3DFACE, got {:?}", db.entities[0]);
    };
    assert_eq!((f.corner1.z, f.corner2.z, f.corner3.z), (1.0, 2.0, 3.0));
    assert_eq!(f.corner4, f.corner3, "three corners given");
}

#[test]
fn a_point_without_a_position_is_kept_and_reported() {
    let db = read_str(&one("POINT", &[])).unwrap();
    assert!(matches!(db.entities[0], Entity::Point(_)));
    assert!(db
        .read_diagnostics
        .warnings
        .iter()
        .any(|w| w.contains("POINT carries no position (group 10)")));
}

#[test]
fn a_3dface_reads_which_of_its_edges_are_invisible() {
    let text = one(
        "3DFACE",
        &[
            (10, "0"),
            (20, "0"),
            (11, "1"),
            (21, "0"),
            (12, "1"),
            (22, "1"),
            (13, "0"),
            (23, "1"),
            (70, "5"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Face3D(f) = &db.entities[0] else {
        panic!("a 3DFACE, got {:?}", db.entities[0]);
    };
    assert_eq!(f.invisible_edges, [true, false, true, false]);

    // Without the group, every edge shows.
    let db = read_str(&one("3DFACE", &[(10, "0"), (11, "1"), (12, "1")])).unwrap();
    let Entity::Face3D(f) = &db.entities[0] else {
        panic!("a 3DFACE");
    };
    assert_eq!(f.invisible_edges, [false; 4]);
}

#[test]
fn a_solid_reads_its_elevation_and_extrusion() {
    let text = one(
        "SOLID",
        &[
            (10, "0"),
            (20, "0"),
            (30, "4"),
            (11, "1"),
            (21, "0"),
            (31, "4"),
            (12, "0"),
            (22, "1"),
            (32, "4"),
            (210, "0"),
            (220, "0"),
            (230, "-1"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Solid(s) = &db.entities[0] else {
        panic!("a SOLID, got {:?}", db.entities[0]);
    };
    assert_eq!(s.elevation, 4.0);
    assert_eq!(
        (s.extrusion.x, s.extrusion.y, s.extrusion.z),
        (0.0, 0.0, -1.0)
    );
}
