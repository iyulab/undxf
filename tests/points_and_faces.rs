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

#[test]
fn an_entitys_colour_is_its_own_not_a_subclass_group_with_the_same_code() {
    // A section object's own subclass writes a group 62 (its indicator
    // colour); the entity's colour is the AcDbEntity part's, here absent --
    // ByLayer.
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nSECTIONOBJECT\n  5\n228\n100\nAcDbEntity\n  8\n0\n100\nAcDbSection\n 90\n1\n 62\n9\n  0\nENDSEC\n  0\nEOF\n";
    let db = undxf::read_str(text).unwrap();
    assert_eq!(db.entities[0].common().color_index, 256);
}

/// A polyface mesh: positions (70 = 192) and faces (70 = 128) in one VERTEX
/// chain. Each face's corners become edges in order and back to the first;
/// a negative index (an invisible edge) is the same corner, a 0 is an unused
/// one, and an index past the positions draws nothing.
#[test]
fn a_polyface_mesh_reads_as_the_wireframe_of_its_faces() {
    let vertex = |groups: &str| format!("  0\nVERTEX\n  8\n0\n{groups}");
    let position = |x: u8, y: u8| vertex(&format!(" 10\n{x}\n 20\n{y}\n 30\n0\n 70\n192\n"));
    let face = |a: i32, b: i32, c: i32, d: i32| {
        vertex(&format!(
            " 10\n0\n 20\n0\n 30\n0\n 70\n128\n 71\n{a}\n 72\n{b}\n 73\n{c}\n 74\n{d}\n"
        ))
    };
    let text = [
        "  0\nSECTION\n  2\nENTITIES\n  0\nPOLYLINE\n  5\n2A\n  8\n0\n 66\n1\n 70\n64\n 71\n4\n 72\n2\n"
            .to_string(),
        position(0, 0),
        position(1, 0),
        position(1, 1),
        position(0, 1),
        // A quad whose last edge is invisible, then a triangle with an
        // unused fourth corner, then a face naming a fifth position.
        face(1, 2, 3, -4),
        face(1, 3, 4, 0),
        face(1, 5, 0, 0),
        "  0\nSEQEND\n  8\n0\n  0\nENDSEC\n  0\nEOF\n".to_string(),
    ]
    .concat();
    let db = read_str(&text).unwrap();
    let Entity::PolylinePFace(mesh) = &db.entities[0] else {
        panic!("a polyface mesh, got {:?}", db.entities[0]);
    };
    let xy = |e: &[uncad_model::model::Point3D; 2]| ((e[0].x, e[0].y), (e[1].x, e[1].y));
    let edges: Vec<_> = mesh.wireframe_edges.iter().map(xy).collect();
    assert_eq!(
        edges,
        [
            ((0.0, 0.0), (1.0, 0.0)),
            ((1.0, 0.0), (1.0, 1.0)),
            ((1.0, 1.0), (0.0, 1.0)),
            ((0.0, 1.0), (0.0, 0.0)),
            ((0.0, 0.0), (1.0, 1.0)),
            ((1.0, 1.0), (0.0, 1.0)),
            ((0.0, 1.0), (0.0, 0.0)),
        ]
    );
    assert_eq!(mesh.skipped_edges, 0);
}
