//! ELLIPSE, SPLINE, MTEXT and the invisible flag: what each group becomes, and what an absent one
//! means.

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
fn an_ellipse_reads_its_axes_and_its_parameters_in_radians() {
    let text = one(
        "ELLIPSE",
        &[
            (10, "1"),
            (20, "2"),
            (30, "0"),
            (11, "3"),
            (21, "0"),
            (31, "0"),
            (40, "0.5"),
            (41, "0.25"),
            (42, "1.5"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Ellipse(e) = &db.entities[0] else {
        panic!("an ELLIPSE, got {:?}", db.entities[0]);
    };
    assert_eq!((e.center.x, e.center.y), (1.0, 2.0));
    assert_eq!(e.major_axis_endpoint.x, 3.0);
    assert_eq!(e.axis_ratio, 0.5);
    // Parameters are written in radians, unlike ARC's degrees.
    assert_eq!((e.start_angle, e.end_angle), (0.25, 1.5));
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn an_ellipse_without_its_parameters_is_kept_whole_and_reported() {
    let text = one(
        "ELLIPSE",
        &[(10, "0"), (20, "0"), (11, "1"), (21, "0"), (40, "1")],
    );
    let db = read_str(&text).unwrap();
    let Entity::Ellipse(e) = &db.entities[0] else {
        panic!("an ELLIPSE");
    };
    assert_eq!((e.start_angle, e.end_angle), (0.0, std::f64::consts::TAU));
    let warnings = &db.read_diagnostics.warnings;
    assert!(warnings
        .iter()
        .any(|w| w.contains("start parameter (group 41)")));
    assert!(warnings
        .iter()
        .any(|w| w.contains("end parameter (group 42)")));
}

#[test]
fn a_spline_by_control_points_reads_its_whole_definition() {
    let text = one(
        "SPLINE",
        &[
            (70, "8"),
            (71, "2"),
            (72, "6"),
            (73, "3"),
            (74, "0"),
            (40, "0"),
            (40, "0"),
            (40, "0"),
            (40, "1"),
            (40, "1"),
            (40, "1"),
            (41, "1"),
            (41, "0.7071067811865476"),
            (41, "1"),
            (10, "1"),
            (20, "0"),
            (30, "0"),
            (10, "1"),
            (20, "1"),
            (30, "0"),
            (10, "0"),
            (20, "1"),
            (30, "0"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Spline(s) = &db.entities[0] else {
        panic!("a SPLINE, got {:?}", db.entities[0]);
    };
    assert_eq!(s.degree, 2);
    // 8 is "planar": neither closed nor periodic, and the record says so.
    assert_eq!((s.closed, s.periodic), (Some(false), Some(false)));
    assert_eq!(s.knots, [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(s.weights, [1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0]);
    assert_eq!(s.control_points.len(), 3);
    assert_eq!((s.control_points[1].x, s.control_points[1].y), (1.0, 1.0));
    assert!(s.fit_points.is_empty());
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn a_spline_that_states_no_flags_and_no_weights_is_unstated_and_not_rational() {
    let text = one(
        "SPLINE",
        &[
            (71, "3"),
            (11, "0"),
            (21, "0"),
            (31, "0"),
            (11, "2"),
            (21, "1"),
            (31, "0"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::Spline(s) = &db.entities[0] else {
        panic!("a SPLINE");
    };
    assert_eq!((s.closed, s.periodic), (None, None));
    assert!(s.weights.is_empty(), "no weights: every weight is 1");
    assert_eq!(s.fit_points.len(), 2);
    assert!(s.control_points.is_empty() && s.knots.is_empty());
}

#[test]
fn an_entity_marked_invisible_reads_as_invisible_and_one_not_marked_as_visible() {
    let hidden = one("CIRCLE", &[(60, "1"), (10, "0"), (20, "0"), (40, "1")]);
    let shown = one("CIRCLE", &[(10, "0"), (20, "0"), (40, "1")]);
    assert!(read_str(&hidden).unwrap().entities[0].common().invisible);
    assert!(!read_str(&shown).unwrap().entities[0].common().invisible);
}

#[test]
fn an_mtext_reads_its_pieces_its_direction_and_its_attachment() {
    let text = one(
        "MTEXT",
        &[
            (10, "1"),
            (20, "2"),
            (40, "2.5"),
            (71, "5"),
            (3, "first piece, "),
            (1, "last piece"),
            (11, "0"),
            (21, "1"),
            (31, "0"),
        ],
    );
    let db = read_str(&text).unwrap();
    let Entity::MText(m) = &db.entities[0] else {
        panic!("an MTEXT, got {:?}", db.entities[0]);
    };
    assert_eq!(m.text, "first piece, last piece");
    assert_eq!(m.text_height, 2.5);
    assert_eq!(m.rotation, std::f64::consts::FRAC_PI_2);
    assert_eq!(
        m.line_spacing_factor, 1.0,
        "no group 44: the default spacing"
    );
    assert_eq!(
        m.attachment,
        Some(uncad_model::model::MTextAttachment::MiddleCenter)
    );
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn an_mtext_that_states_no_attachment_and_no_direction_is_unstated_and_level() {
    let text = one("MTEXT", &[(10, "0"), (20, "0"), (40, "1"), (1, "x")]);
    let db = read_str(&text).unwrap();
    let Entity::MText(m) = &db.entities[0] else {
        panic!("an MTEXT");
    };
    assert_eq!((m.attachment, m.rotation), (None, 0.0));
}
