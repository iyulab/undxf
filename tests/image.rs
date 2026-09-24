//! IMAGE, its frame and clip boundary, and the IMAGEDEF it names.

use uncad_model::model::{Entity, ImageEntity, Point2D, Point3D, Ref};
use uncad_model::tables::ResolutionUnit;
use undxf::read_str;

fn drawing(image: &[(i32, &str)], objects: &str) -> uncad_model::CadDatabase {
    let mut text =
        String::from("  0\nSECTION\n  2\nENTITIES\n  0\nIMAGE\n  5\n2A\n100\nAcDbEntity\n  8\n0\n");
    for (code, value) in image {
        text.push_str(&format!("{code:>3}\n{value}\n"));
    }
    text.push_str("  0\nENDSEC\n  0\nSECTION\n  2\nOBJECTS\n");
    text.push_str(objects);
    text.push_str("  0\nENDSEC\n  0\nEOF\n");
    read_str(&text).unwrap()
}

const FRAME: &[(i32, &str)] = &[
    (100, "AcDbRasterImage"),
    (90, "0"),
    (10, "10"),
    (20, "5"),
    (30, "0"),
    (11, "0.5"),
    (21, "0"),
    (31, "0"),
    (12, "0"),
    (22, "0.5"),
    (32, "0"),
    (13, "40"),
    (23, "20"),
    (340, "80F"),
    (70, "7"),
    (280, "1"),
    (281, "50"),
    (282, "60"),
    (283, "10"),
];

const DEFINITION: &str = "  0\nIMAGEDEF\n  5\n80F\n100\nAcDbRasterImageDef\n 90\n0\n  1\nimages/plan.png\n 10\n40\n 20\n20\n 11\n0.1\n 21\n0.1\n280\n1\n281\n2\n";

fn image(db: &uncad_model::CadDatabase) -> &ImageEntity {
    match &db.entities[0] {
        Entity::Image(i) => i,
        other => panic!("an IMAGE, not {other:?}"),
    }
}

#[test]
fn an_image_carries_its_frame_its_settings_and_its_definition() {
    let mut groups = FRAME.to_vec();
    groups.extend([
        (71, "1"),
        (91, "2"),
        (14, "-0.5"),
        (24, "-0.5"),
        (14, "39.5"),
        (24, "19.5"),
    ]);
    let db = drawing(&groups, DEFINITION);
    assert!(
        db.read_diagnostics.warnings.is_empty(),
        "{:?}",
        db.read_diagnostics.warnings
    );
    let i = image(&db);
    assert_eq!(
        i.insertion_point,
        Point3D {
            x: 10.0,
            y: 5.0,
            z: 0.0
        }
    );
    assert_eq!(
        i.u_vector,
        Point3D {
            x: 0.5,
            y: 0.0,
            z: 0.0
        }
    );
    assert_eq!(i.size_pixels, Point2D { x: 40.0, y: 20.0 });
    assert_eq!(i.definition, Ref::Resolved("80F".to_string()));
    assert_eq!(
        (
            i.display_flags,
            i.clipping,
            i.brightness,
            i.contrast,
            i.fade
        ),
        (Some(7), Some(true), Some(50), Some(60), Some(10))
    );
    // The flag is a variable of R2010 on; this file does not write it.
    assert_eq!(i.clip_outside, None);
    // A rectangle over the whole image is the frame's own outline.
    assert_eq!(
        i.boundary,
        vec![
            Point2D { x: 10.0, y: 15.0 },
            Point2D { x: 30.0, y: 15.0 },
            Point2D { x: 30.0, y: 5.0 },
            Point2D { x: 10.0, y: 5.0 },
        ]
    );
    let d = &db.tables.image_definitions["80F"];
    assert_eq!(d.file_path.as_deref(), Some("images/plan.png"));
    assert_eq!(d.size_pixels, Point2D { x: 40.0, y: 20.0 });
    assert_eq!(d.pixel_size, Point2D { x: 0.1, y: 0.1 });
    assert_eq!(d.loaded, Some(true));
    assert_eq!(d.resolution_unit, Some(ResolutionUnit::Centimeter));
}

#[test]
fn a_definition_nothing_answers_to_keeps_the_handle() {
    let db = drawing(FRAME, "");
    assert_eq!(image(&db).definition, Ref::Unresolved("80F".to_string()));
    assert!(db.tables.image_definitions.is_empty());
}

#[test]
fn no_clip_boundary_is_the_whole_image_and_the_2010_flag_is_read() {
    let mut groups = FRAME.to_vec();
    groups.push((290, "1"));
    let db = drawing(&groups, DEFINITION);
    let i = image(&db);
    assert_eq!(i.clip_outside, Some(true));
    assert_eq!(i.boundary.len(), 4);
    assert_eq!(i.boundary[1], Point2D { x: 30.0, y: 15.0 });
}
