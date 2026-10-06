//! A MULTILEADER's leader roots: each `LEADER{}` block of its context data,
//! with the points inside each of its `LEADER_LINE{}` blocks, its last
//! leader line point and its dogleg -- and nothing else the record says
//! with the same group codes; and the text it points out.

use uncad_model::model::{Entity, MultiLeaderContent};
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

/// The line spacing of a multileader's text is its context data's 45, and
/// 1 -- the default spacing -- when the record leaves the group out.
#[test]
fn a_multileader_text_keeps_its_line_spacing() {
    let read = |spacing: Option<&str>| {
        let mut groups: Vec<(i32, &str)> = vec![
            (100, "AcDbMLeader"),
            (300, "CONTEXT_DATA{"),
            (41, "2.5"),
            (290, "1"),
            (304, r"A\PB"),
            (12, "3"),
            (22, "4"),
            (32, "0"),
        ];
        if let Some(spacing) = spacing {
            groups.push((45, spacing));
        }
        groups.extend([(171, "1"), (296, "0"), (301, "}")]);
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
        match &m.content {
            Some(MultiLeaderContent::MText(t)) => t.line_spacing_factor,
            other => panic!("a text, got {other:?}"),
        }
    };
    assert_eq!(read(Some("1.5")), 1.5);
    assert_eq!(read(None), 1.0);
}

/// The arrowheads of a multileader are drawn at its context data's
/// arrowhead size (140), unless a line's override flags (93) set bit 0x10
/// and the line states its own (40) -- not the root's dogleg length, which
/// is a 40 too.
#[test]
fn a_multileader_reads_the_size_its_arrowheads_are_drawn_at() {
    let read = |line_flags: &str, line_size: &str| {
        let groups: Vec<(i32, &str)> = vec![
            (100, "AcDbMLeader"),
            (300, "CONTEXT_DATA{"),
            (40, "1"),
            (41, "0.18"),
            (140, "0.18"),
            (302, "LEADER{"),
            (290, "1"),
            (291, "1"),
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
            (91, "0"),
            (170, "1"),
            (40, line_size),
            (93, line_flags),
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
        m.arrow_size
    };
    // The line does not override its size: the context data's.
    assert_eq!(read("0", "0.5"), Some(0.18));
    // It does: its own.
    assert_eq!(read("16", "0.5"), Some(0.5));
}
