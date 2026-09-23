//! What a VIEWPORT record states about its view and its state, read from
//! hand-written records: the corpus has no viewport that is off, and the
//! golden sheet writes its two ways of saying "on" in agreement.

use uncad_model::model::{Entity, Ref, ViewportEntity};
use undxf::read_str;

fn read_one(tables: &str, entity: &str) -> ViewportEntity {
    let text = format!(
        "  0\nSECTION\n  2\nTABLES\n{tables}  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n{entity}  0\nENDSEC\n  0\nEOF\n"
    );
    let db = read_str(&text).expect("the drawing reads");
    match db.entities.first().expect("one entity") {
        Entity::Viewport(v) => v.clone(),
        other => panic!("expected a viewport, got {other:?}"),
    }
}

fn viewport(groups: &str) -> String {
    format!("  0\nVIEWPORT\n  5\n2A\n 67\n1\n  8\n0\n 10\n100.0\n 20\n50.0\n 40\n200.0\n 41\n100.0\n{groups}")
}

/// Group 68 is the viewport's place in the stack of active viewports, and a
/// viewport of a layout that is not the current one is written with 0 there
/// whether it is on or not. From R2000 on the status flags (90) say it: bit
/// 0x20000 set is off.
#[test]
fn the_status_flags_say_whether_a_viewport_is_on() {
    let inactive_layout = read_one("", &viewport(" 68\n0\n 69\n0\n 90\n32800\n"));
    assert_eq!(inactive_layout.on, Some(true));
    let off = read_one("", &viewport(" 68\n0\n 69\n2\n 90\n163872\n"));
    assert_eq!(off.on, Some(false));
    // A record older than R2000 has no status flags: 68 is all there is.
    assert_eq!(read_one("", &viewport(" 68\n0\n")).on, Some(false));
    assert_eq!(read_one("", &viewport(" 68\n1\n")).on, Some(true));
    assert_eq!(read_one("", &viewport("")).on, None);
}

/// The frozen layers are named by their LAYER records' handles, and read as
/// the layers' names; a handle no layer carries is kept as written.
#[test]
fn frozen_layers_resolve_from_handles_to_names() {
    let layers =
        "  0\nTABLE\n  2\nLAYER\n  0\nLAYER\n  5\n1F\n  2\nWALLS\n 70\n0\n 62\n7\n  0\nENDTAB\n";
    let v = read_one(layers, &viewport(" 341\n1f\n 341\n99\n"));
    assert_eq!(
        v.frozen_layers,
        [
            Ref::Resolved("WALLS".to_string()),
            Ref::Unresolved("99".to_string())
        ]
    );
}

/// A record that writes no view centre carries no view; a zero view
/// direction is not a direction and reads as a plan view.
#[test]
fn a_view_is_read_when_the_record_states_one() {
    assert_eq!(read_one("", &viewport("")).view, None);
    let v = read_one(
        "",
        &viewport(" 12\n5.0\n 22\n6.0\n 16\n0.0\n 26\n0.0\n 36\n0.0\n 17\n1.0\n 27\n2.0\n 37\n0.0\n 45\n40.0\n 51\n90.0\n"),
    )
    .view
    .expect("a view");
    assert_eq!((v.center.x, v.center.y), (5.0, 6.0));
    assert_eq!(
        (v.direction.x, v.direction.y, v.direction.z),
        (0.0, 0.0, 1.0)
    );
    assert_eq!((v.target.x, v.target.y), (1.0, 2.0));
    assert_eq!(v.height, 40.0);
    assert!((v.twist - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
}
