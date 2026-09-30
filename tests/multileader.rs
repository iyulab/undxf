//! A MULTILEADER's leader roots: each `LEADER{}` block of its context data,
//! with the points inside each of its `LEADER_LINE{}` blocks, its last
//! leader line point and its dogleg -- and nothing else the record says
//! with the same group codes.

use uncad_model::model::Entity;
use undxf::read_str;

#[test]
fn a_multileader_reads_the_points_of_each_leader_line_block() {
    let groups: &[(i32, &str)] = &[
        (100, "AcDbMLeader"),
        (300, "CONTEXT_DATA{"),
        // The content's own base point: not a leader line.
        (10, "5"),
        (20, "5"),
        (30, "0"),
        (302, "LEADER{"),
        (290, "1"),
        (291, "1"),
        // The root's last leader line point: where its lines run to.
        (10, "4"),
        (20, "5"),
        (30, "0"),
        (11, "1"),
        (21, "0"),
        (31, "0"),
        (90, "0"),
        (40, "0.36"),
        (304, "LEADER_LINE{"),
        (10, "0"),
        (20, "0"),
        (30, "0"),
        (10, "2"),
        (20, "3"),
        (30, "0"),
        (91, "0"),
        (305, "}"),
        (304, "LEADER_LINE{"),
        (10, "0"),
        (20, "9"),
        (30, "1"),
        (91, "1"),
        (305, "}"),
        // A line block with no point is no line.
        (304, "LEADER_LINE{"),
        (91, "2"),
        (305, "}"),
        (303, "}"),
        // A second root that states neither a last point nor a dogleg.
        (302, "LEADER{"),
        (290, "0"),
        (291, "0"),
        (10, "7"),
        (20, "7"),
        (30, "0"),
        (304, "LEADER_LINE{"),
        (10, "8"),
        (20, "8"),
        (30, "0"),
        (305, "}"),
        (303, "}"),
        (301, "}"),
    ];
    let mut text = String::from(
        "  0\nSECTION\n  2\nENTITIES\n  0\nMULTILEADER\n  5\n2A\n100\nAcDbEntity\n  8\n0\n",
    );
    for (code, value) in groups {
        text.push_str(&format!("{code:>3}\n{value}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    let db = read_str(&text).unwrap();
    let Entity::MultiLeader(m) = &db.entities[0] else {
        panic!("a MULTILEADER, got {:?}", db.entities[0]);
    };
    assert_eq!(m.leaders.len(), 2);
    let first = &m.leaders[0];
    let lines: Vec<Vec<(f64, f64, f64)>> = first
        .lines
        .iter()
        .map(|l| l.iter().map(|p| (p.x, p.y, p.z)).collect())
        .collect();
    assert_eq!(
        lines,
        [
            vec![(0.0, 0.0, 0.0), (2.0, 3.0, 0.0)],
            vec![(0.0, 9.0, 1.0)]
        ]
    );
    let last = first.last_point.expect("the root states its last point");
    assert_eq!((last.x, last.y, last.z), (4.0, 5.0, 0.0));
    let dogleg = first.dogleg.expect("the root states its dogleg");
    assert_eq!(
        (dogleg.direction.x, dogleg.direction.y, dogleg.length),
        (1.0, 0.0, 0.36)
    );
    // Flags off: the values the root wrote anyway are not its.
    let second = &m.leaders[1];
    assert_eq!(second.lines.len(), 1);
    assert_eq!(second.last_point, None);
    assert_eq!(second.dogleg, None);
}
