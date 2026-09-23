//! How a line of text is placed: TEXT, ATTRIB and ATTDEF alignment -- the two
//! alignment codes, the alignment point the format writes only for an aligned
//! text, and the width factor -- and the width of the box an MTEXT wraps in.

use uncad_model::model::{Entity, Point2D, TextHorizontalAlignment, TextVerticalAlignment};
use undxf::read_str;

/// A DXF with one TEXT carrying `groups` after its layer, in order.
fn text(groups: &[(i32, &str)]) -> String {
    let mut out = String::from("  0\nSECTION\n  2\nENTITIES\n  0\nTEXT\n  5\n2A\n  8\n0\n");
    for (code, value) in groups {
        out.push_str(&format!("{code:>3}\n{value}\n"));
    }
    out.push_str("  0\nENDSEC\n  0\nEOF\n");
    out
}

const BASE: [(i32, &str); 5] = [(10, "1"), (20, "2"), (30, "0"), (40, "2.5"), (1, "A-1")];

fn read(extra: &[(i32, &str)]) -> (uncad_model::model::TextEntity, Vec<String>) {
    let groups: Vec<_> = BASE.iter().chain(extra).copied().collect();
    let db = read_str(&text(&groups)).unwrap();
    let Entity::Text(t) = &db.entities[0] else {
        panic!("a TEXT, got {:?}", db.entities[0]);
    };
    (t.clone(), db.read_diagnostics.warnings.clone())
}

#[test]
fn a_text_that_states_no_alignment_is_left_and_baseline_at_its_normal_width() {
    let (t, warnings) = read(&[]);
    assert_eq!(
        (t.horizontal_alignment, t.vertical_alignment),
        (
            TextHorizontalAlignment::Left,
            TextVerticalAlignment::Baseline
        )
    );
    assert_eq!(t.alignment_point, None);
    assert_eq!(t.width_factor, 1.0);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn an_aligned_text_carries_its_alignment_point_and_width() {
    let (t, warnings) = read(&[(41, "0.8"), (72, "2"), (11, "9"), (21, "2"), (73, "3")]);
    assert_eq!(
        (t.horizontal_alignment, t.vertical_alignment),
        (TextHorizontalAlignment::Right, TextVerticalAlignment::Top)
    );
    assert_eq!(t.alignment_point, Some(Point2D { x: 9.0, y: 2.0 }));
    assert_eq!(t.width_factor, 0.8);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn an_alignment_point_on_a_left_baseline_text_is_not_one() {
    // The format writes 11 only for an aligned text; one written anyway
    // says nothing about where a left, baseline text goes.
    let (t, _) = read(&[(11, "9"), (21, "2")]);
    assert_eq!(t.alignment_point, None);
}

#[test]
fn an_alignment_outside_the_format_is_reported_and_read_as_the_default() {
    let (t, warnings) = read(&[(72, "7"), (73, "9")]);
    assert_eq!(
        (t.horizontal_alignment, t.vertical_alignment),
        (
            TextHorizontalAlignment::Left,
            TextVerticalAlignment::Baseline
        )
    );
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().all(|w| w.starts_with("TEXT_ALIGNMENT:")));
}

#[test]
fn an_attribute_reads_its_vertical_alignment_from_74_not_73() {
    // In an ATTRIB, 73 is the field length; the vertical alignment is 74.
    let dxf = "  0\nSECTION\n  2\nENTITIES\n  0\nATTRIB\n  5\n2B\n  8\n0\n 10\n1\n 20\n2\n 40\n2.5\n  1\nBP-1042\n 41\n0.9\n 72\n2\n 11\n40\n 21\n3\n  2\nDWGNO\n 70\n0\n 73\n12\n 74\n2\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(dxf).unwrap();
    let Entity::Attrib(a) = &db.entities[0] else {
        panic!("an ATTRIB, got {:?}", db.entities[0]);
    };
    assert_eq!(
        (a.horizontal_alignment, a.vertical_alignment),
        (
            TextHorizontalAlignment::Right,
            TextVerticalAlignment::Middle
        )
    );
    assert_eq!(a.alignment_point, Some(Point2D { x: 40.0, y: 3.0 }));
    assert_eq!(a.width_factor, 0.9);
    assert!(db.read_diagnostics.is_clean(), "{:?}", db.read_diagnostics);
}

#[test]
fn an_mtext_carries_the_width_of_the_box_it_wraps_in() {
    let mtext = |width: Option<&str>| {
        let mut groups = String::from(
            "  0\nSECTION\n  2\nENTITIES\n  0\nMTEXT\n  5\n2C\n  8\n0\n 10\n0\n 20\n0\n 40\n2.5\n",
        );
        if let Some(w) = width {
            groups.push_str(&format!(" 41\n{w}\n"));
        }
        groups.push_str("  1\nA long note\n  0\nENDSEC\n  0\nEOF\n");
        let db = read_str(&groups).unwrap();
        let Entity::MText(m) = &db.entities[0] else {
            panic!("an MTEXT, got {:?}", db.entities[0]);
        };
        m.reference_width
    };
    assert_eq!(mtext(Some("60.5")), 60.5);
    // No box: each paragraph is one line.
    assert_eq!(mtext(None), 0.0);
}
