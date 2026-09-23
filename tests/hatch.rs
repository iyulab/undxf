//! HATCH, read in order: boundary paths of every kind, the pattern, the
//! gradient, and a record that stops short of what it states.

use uncad_model::model::{Entity, HatchBoundaryPath, HatchEdge, HatchEntity, Point2D};
use undxf::read_str;

/// A DXF holding one HATCH whose own groups (after its subclass marker) are
/// `groups`, in order.
fn hatch(groups: &[(i32, &str)]) -> (HatchEntity, Vec<String>) {
    let mut text = String::from(
        "  0\nSECTION\n  2\nENTITIES\n  0\nHATCH\n  5\n2D\n  8\n0\n100\nAcDbEntity\n100\nAcDbHatch\n",
    );
    for (code, value) in groups {
        text.push_str(&format!("{code:>3}\n{value}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let db = read_str(&text).unwrap();
    let Entity::Hatch(h) = &db.entities[0] else {
        panic!("a HATCH, got {:?}", db.entities[0]);
    };
    (h.clone(), db.read_diagnostics.warnings.clone())
}

const HEAD: [(i32, &str); 8] = [
    (10, "0"),
    (20, "0"),
    (30, "0"),
    (210, "0"),
    (220, "0"),
    (230, "1"),
    (2, "ANSI31"),
    (71, "0"),
];

fn p(x: f64, y: f64) -> Point2D {
    Point2D { x, y }
}

#[test]
fn a_polyline_path_keeps_its_bulges_and_a_solid_fill_its_flag() {
    let mut g: Vec<(i32, &str)> = HEAD.to_vec();
    g.extend([
        (70, "1"),
        (91, "1"),
        (92, "2"),
        (72, "1"),
        (73, "1"),
        (93, "3"),
        (10, "0"),
        (20, "0"),
        (42, "0"),
        (10, "4"),
        (20, "0"),
        (42, "1"),
        (10, "4"),
        (20, "4"),
        (42, "0"),
        (97, "0"),
        (75, "0"),
        (76, "1"),
        (98, "0"),
    ]);
    let (h, warnings) = hatch(&g);
    assert!(h.solid_fill);
    let [HatchBoundaryPath::Polyline(v)] = h.boundary_paths.as_slice() else {
        panic!("{:?}", h.boundary_paths);
    };
    assert_eq!(v.len(), 3);
    assert_eq!((v[1].point, v[1].bulge), (p(4.0, 0.0), 1.0));
    assert!(h.pattern_lines.is_empty() && h.gradient.is_none());
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn an_edge_path_reads_each_kind_of_edge_with_its_angles_in_radians() {
    let mut g: Vec<(i32, &str)> = HEAD.to_vec();
    g.extend([
        (70, "0"),
        (91, "1"),
        (92, "1"),
        (93, "4"),
        // A line from (0, 0) to (4, 0).
        (72, "1"),
        (10, "0"),
        (20, "0"),
        (11, "4"),
        (21, "0"),
        // An arc about (4, 2), radius 2, from -90 to 90 degrees.
        (72, "2"),
        (10, "4"),
        (20, "2"),
        (40, "2"),
        (50, "-90"),
        (51, "90"),
        (73, "1"),
        // An elliptical arc about (2, 4), half of it, clockwise.
        (72, "3"),
        (10, "2"),
        (20, "4"),
        (11, "2"),
        (21, "0"),
        (40, "0.5"),
        (50, "0"),
        (51, "180"),
        (73, "0"),
        // A rational spline back to the start, by its three control points.
        (72, "4"),
        (94, "2"),
        (73, "1"),
        (74, "0"),
        (95, "6"),
        (96, "3"),
        (40, "0"),
        (40, "0"),
        (40, "0"),
        (40, "1"),
        (40, "1"),
        (40, "1"),
        (10, "0"),
        (20, "4"),
        (10, "-1"),
        (20, "2"),
        (10, "0"),
        (20, "0"),
        (42, "1"),
        (42, "0.7"),
        (42, "1"),
        (97, "0"),
        (97, "0"),
        (75, "0"),
        (76, "1"),
        (98, "0"),
    ]);
    let (h, warnings) = hatch(&g);
    assert!(warnings.is_empty(), "{warnings:?}");
    let [HatchBoundaryPath::Edges(e)] = h.boundary_paths.as_slice() else {
        panic!("{:?}", h.boundary_paths);
    };
    assert_eq!(e.len(), 4);
    assert_eq!(e[0], HatchEdge::Line { start: p(0.0, 0.0) });
    let HatchEdge::Arc {
        center,
        radius,
        start_angle,
        end_angle,
        is_ccw,
    } = e[1]
    else {
        panic!("{:?}", e[1]);
    };
    assert_eq!((center, radius, is_ccw), (p(4.0, 2.0), 2.0, true));
    assert!((start_angle + std::f64::consts::FRAC_PI_2).abs() < 1e-15);
    assert!((end_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-15);
    let HatchEdge::Ellipse {
        end,
        minor_major_ratio,
        end_angle,
        is_ccw,
        ..
    } = e[2]
    else {
        panic!("{:?}", e[2]);
    };
    assert_eq!((end, minor_major_ratio, is_ccw), (p(2.0, 0.0), 0.5, false));
    assert!((end_angle - std::f64::consts::PI).abs() < 1e-15);
    assert_eq!(
        e[3],
        HatchEdge::Spline {
            control_points: vec![p(0.0, 4.0), p(-1.0, 2.0), p(0.0, 0.0)]
        }
    );
}

#[test]
fn pattern_lines_pair_their_base_point_and_offset_groups() {
    let mut g: Vec<(i32, &str)> = HEAD.to_vec();
    g.extend([
        (70, "0"),
        (91, "0"),
        (75, "0"),
        (76, "1"),
        (52, "0"),
        (41, "1"),
        (77, "0"),
        (78, "1"),
        (53, "45"),
        (43, "1"),
        (44, "2"),
        (45, "-2.2"),
        (46, "2.2"),
        (79, "2"),
        (49, "3"),
        (49, "-1"),
        (98, "0"),
    ]);
    let (h, warnings) = hatch(&g);
    assert!(warnings.is_empty(), "{warnings:?}");
    let [line] = h.pattern_lines.as_slice() else {
        panic!("{:?}", h.pattern_lines);
    };
    assert!((line.angle - std::f64::consts::FRAC_PI_4).abs() < 1e-15);
    assert_eq!((line.base_point, line.offset), (p(1.0, 2.0), p(-2.2, 2.2)));
    assert_eq!(line.dash_pattern, vec![3.0, -1.0]);
}

#[test]
fn a_gradient_carries_its_stops_in_order_and_its_name_says_radial() {
    let mut g: Vec<(i32, &str)> = HEAD.to_vec();
    g.extend([
        (70, "1"),
        (91, "0"),
        (75, "0"),
        (76, "1"),
        (98, "0"),
        (450, "1"),
        (451, "0"),
        (452, "0"),
        (453, "2"),
        (463, "1"),
        (63, "5"),
        (421, "255"),
        (463, "0"),
        (63, "1"),
        (421, "16711680"),
        (460, "0.5"),
        (461, "0"),
        (462, "0"),
        (470, "SPHERICAL"),
    ]);
    let (h, _) = hatch(&g);
    let gradient = h.gradient.expect("a gradient");
    // Ordered by position: the red stop (0) first, the blue one (1) last.
    assert_eq!(
        (gradient.color1, gradient.color2),
        (0xff0000, Some(0x0000ff))
    );
    assert!(gradient.is_radial);
    assert_eq!(gradient.angle, 0.5);
}

#[test]
fn a_path_that_stops_short_of_its_count_keeps_what_was_read_and_says_so() {
    let mut g: Vec<(i32, &str)> = HEAD.to_vec();
    g.extend([
        (70, "1"),
        (91, "2"),
        (92, "2"),
        (72, "0"),
        (73, "1"),
        (93, "3"),
        (10, "0"),
        (20, "0"),
        (10, "4"),
        (20, "0"),
        (97, "0"),
        (75, "0"),
    ]);
    let (h, warnings) = hatch(&g);
    assert!(h.boundary_paths.is_empty(), "{:?}", h.boundary_paths);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("HATCH_STRUCTURE:"), "{warnings:?}");
}

#[test]
fn the_elevation_and_extrusion_come_from_the_header_and_default_when_absent() {
    use uncad_model::model::Point3D;
    let groups = [
        (10, "0"),
        (20, "0"),
        (30, "2.5"),
        (210, "0"),
        (220, "0"),
        (230, "-1"),
        (2, "SOLID"),
        (70, "1"),
        (71, "0"),
        (91, "0"),
    ];
    let (h, warnings) = hatch(&groups);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(h.elevation, 2.5);
    assert_eq!(
        h.extrusion,
        Point3D {
            x: 0.0,
            y: 0.0,
            z: -1.0
        }
    );
    // A writer leaves the default extrusion out.
    let (h, _) = hatch(&[(2, "SOLID"), (70, "1"), (71, "0"), (91, "0")]);
    assert_eq!(h.elevation, 0.0);
    assert_eq!(
        h.extrusion,
        Point3D {
            x: 0.0,
            y: 0.0,
            z: 1.0
        }
    );
}

#[test]
fn the_fill_style_after_the_paths_is_read_and_an_undefined_one_is_none() {
    use uncad_model::model::HatchStyle;
    let with_style = |code: &'static str| {
        hatch(&[
            (2, "SOLID"),
            (70, "1"),
            (71, "0"),
            (91, "0"),
            (75, code),
            (76, "1"),
            (98, "0"),
        ])
        .0
        .style
    };
    assert_eq!(with_style("0"), Some(HatchStyle::Normal));
    assert_eq!(with_style("2"), Some(HatchStyle::Ignore));
    assert_eq!(with_style("5"), None);
}
