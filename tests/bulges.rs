//! Polyline bulges (DXF 42): each vertex carries the bulge of the segment that leaves it, and an
//! absent 42 is a straight segment.

use uncad_model::model::Entity;
use undxf::read_str;

#[test]
fn an_lwpolyline_vertex_carries_the_bulge_written_after_it() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nLWPOLYLINE\n  5\n2A\n  8\n0\n\
                 90\n3\n 70\n1\n\
                 10\n0\n 20\n0\n\
                 10\n1\n 20\n0\n 42\n0.5\n\
                 10\n1\n 20\n1\n 42\n-1\n\
                  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    let Entity::LwPolyline(p) = &db.entities[0] else {
        panic!("an LWPOLYLINE, got {:?}", db.entities[0]);
    };
    let bulges: Vec<f64> = p.vertices.iter().map(|v| v.bulge).collect();
    assert_eq!(bulges, [0.0, 0.5, -1.0]);
    assert_eq!((p.vertices[2].point.x, p.vertices[2].point.y), (1.0, 1.0));
    assert!(p.closed);
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn a_2d_polyline_vertex_carries_its_bulge() {
    let text = "  0\nSECTION\n  2\nENTITIES\n\
                  0\nPOLYLINE\n  5\n2A\n  8\n0\n 66\n1\n 70\n0\n\
                  0\nVERTEX\n  5\n2B\n  8\n0\n 10\n0\n 20\n0\n 30\n0\n 42\n0.25\n\
                  0\nVERTEX\n  5\n2C\n  8\n0\n 10\n2\n 20\n0\n 30\n0\n\
                  0\nSEQEND\n  5\n2D\n  8\n0\n\
                  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    let Entity::Polyline2D(p) = &db.entities[0] else {
        panic!("a 2D POLYLINE, got {:?}", db.entities[0]);
    };
    let bulges: Vec<f64> = p.vertices.iter().map(|v| v.bulge).collect();
    assert_eq!(bulges, [0.25, 0.0]);
}
