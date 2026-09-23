//! The first golden case, read by this crate, must be exactly the model the
//! case's spec says -- the same model the other reader of this format
//! produces from the same file.

use uncad_model::model::{Entity, EntityId, LeaderEntity, Ref};
use uncad_model::CadDatabase;
use undxf::{read_bytes, read_str};

fn expected() -> CadDatabase {
    serde_json::from_str(include_str!("golden/g1.expected.json"))
        .expect("the golden model deserializes")
}

#[test]
fn g1_reads_to_exactly_the_expected_model() {
    let db = read_str(include_str!("golden/g1.dxf")).unwrap();
    let expected = expected();
    // Field by field first, so a mismatch names its place.
    assert_eq!(db.tables.layers, expected.tables.layers);
    assert_eq!(
        db.tables.block_records.keys().collect::<Vec<_>>(),
        expected.tables.block_records.keys().collect::<Vec<_>>()
    );
    for (name, block) in &expected.tables.block_records {
        assert_eq!(&db.tables.block_records[name], block, "block {name}");
    }
    assert_eq!(db.entities.len(), expected.entities.len());
    for (got, want) in db.entities.iter().zip(&expected.entities) {
        assert_eq!(got, want);
    }
    assert_eq!(db, expected);
}

#[test]
fn an_unknown_entity_type_is_kept_under_its_name_not_dropped() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nWIDGET\n  5\n2A\n  8\n0\n 10\n1.5\n  0\nLINE\n  5\n2B\n  8\n0\n 10\n0\n 20\n0\n 11\n1\n 21\n1\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    assert_eq!(db.entities.len(), 2);
    match &db.entities[0] {
        Entity::Unknown { common, type_name } => {
            assert_eq!(type_name, "WIDGET");
            assert_eq!(common.source_handle, Ref::Resolved("2A".into()));
            assert_eq!(
                common.layer,
                Ref::Unresolved("0".into()),
                "no LAYER table declared the layer, so the name stays unresolved"
            );
        }
        other => panic!("expected UNKNOWN, got {other:?}"),
    }
    assert!(matches!(db.entities[1], Entity::Line(_)));
}

#[test]
fn a_block_reference_to_no_block_stays_unresolved_with_its_name() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nINSERT\n  5\n30\n  8\n0\n  2\nGHOST\n 10\n0\n 20\n0\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    let Entity::Insert(i) = &db.entities[0] else {
        panic!("an INSERT");
    };
    assert_eq!(i.block_name, Ref::Unresolved("GHOST".into()));
}

#[test]
fn a_truncated_file_is_an_error_that_names_the_line() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n  5\n2B\n 10\n0\n";
    let err = read_str(text).unwrap_err();
    assert!(err.to_string().contains("line"), "{err}");
}

/// Every golden case whose text is ASCII reads to exactly its expected
/// model: nested blocks (G2), two coincident lines (G6), a title block of
/// loose texts (G7), the same title block twice (G9), a mirrored part (G11),
/// curved and wide polylines (G12), justified text with styles and an
/// invisible attribute (G13), a sheet of viewports over layers in every state
/// with its layouts (G14), a polygon mesh (G15), and ordinate dimensions with
/// a style that states every display variable (G16).
#[test]
fn the_other_ascii_cases_read_exactly_too() {
    for (name, dxf, json) in [
        (
            "g2",
            include_str!("golden/g2.dxf"),
            include_str!("golden/g2.expected.json"),
        ),
        (
            "g6",
            include_str!("golden/g6.dxf"),
            include_str!("golden/g6.expected.json"),
        ),
        (
            "g7",
            include_str!("golden/g7.dxf"),
            include_str!("golden/g7.expected.json"),
        ),
        (
            "g9",
            include_str!("golden/g9.dxf"),
            include_str!("golden/g9.expected.json"),
        ),
        (
            "g11",
            include_str!("golden/g11.dxf"),
            include_str!("golden/g11.expected.json"),
        ),
        (
            "g12",
            include_str!("golden/g12.dxf"),
            include_str!("golden/g12.expected.json"),
        ),
        (
            "g13",
            include_str!("golden/g13.dxf"),
            include_str!("golden/g13.expected.json"),
        ),
        (
            "g14",
            include_str!("golden/g14.dxf"),
            include_str!("golden/g14.expected.json"),
        ),
        (
            "g15",
            include_str!("golden/g15.dxf"),
            include_str!("golden/g15.expected.json"),
        ),
        (
            "g16",
            include_str!("golden/g16.dxf"),
            include_str!("golden/g16.expected.json"),
        ),
    ] {
        let expected: CadDatabase = serde_json::from_str(json).expect("deserializes");
        let db = read_str(dxf).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(db, expected, "{name}");
    }
}

/// The CP949 title block (G8): read from bytes, its Korean layer names,
/// attribute values and texts come out exactly as the spec wrote them,
/// and the read is clean.
#[test]
fn g8_decodes_its_code_page_to_exactly_the_expected_model() {
    let expected: CadDatabase =
        serde_json::from_str(include_str!("golden/g8.expected.json")).expect("deserializes");
    let db = read_bytes(include_bytes!("golden/g8.dxf")).unwrap();
    assert_eq!(db.tables.layers, expected.tables.layers);
    assert_eq!(db, expected);
    assert!(db.read_diagnostics.is_clean());
}

#[test]
fn bytes_that_are_valid_utf8_read_the_same_as_the_text() {
    let text = include_str!("golden/g1.dxf");
    assert_eq!(
        read_bytes(text.as_bytes()).unwrap(),
        read_str(text).unwrap()
    );
}

/// A DIMENSION states more than the block it is drawn with, and each value
/// is one DXF group. These are hand-written records rather than corpus
/// files: the corpus has no dimension whose text is `<>` or a single space,
/// and those two spellings are exactly the ones the model folds.
mod dimension {
    use super::*;
    use uncad_model::model::{DimensionKind, TextOverride};

    fn read_one(entity: &str) -> uncad_model::model::DimensionEntity {
        let text = format!("  0\nSECTION\n  2\nENTITIES\n{entity}  0\nENDSEC\n  0\nEOF\n");
        let db = read_str(&text).expect("the drawing reads");
        match db.entities.first().expect("one entity") {
            Entity::Dimension(d) => d.clone(),
            other => panic!("expected a dimension, got {other:?}"),
        }
    }

    /// Groups 13/14/15/16 belong to some subtypes and not others, so their
    /// absence is a fact about the dimension, not a gap to fill with zero.
    #[test]
    fn a_linear_dimension_states_its_measurement_points_and_style() {
        let d = read_one(
            "  0\nDIMENSION\n  5\n2A\n  8\n0\n  2\n*D1\n  3\nISO-25\n 70\n32\n\
             10\n1.0\n 20\n2.0\n 11\n3.0\n 21\n4.0\n 13\n5.0\n 23\n6.0\n\
             14\n7.0\n 24\n8.0\n 42\n9.5\n",
        );
        assert_eq!(d.kind, Some(DimensionKind::Rotated));
        assert_eq!(d.measurement, Some(9.5));
        assert_eq!(d.text_override, TextOverride::Measured);
        assert_eq!(d.definition_point.expect("group 10").x, 1.0);
        assert_eq!(d.text_midpoint.y, 4.0);
        assert_eq!(d.points.extension1.expect("group 13").y, 6.0);
        assert_eq!(d.points.extension2.expect("group 14").x, 7.0);
        assert_eq!(d.points.radial, None);
        assert_eq!(d.points.arc, None);
        assert_eq!(d.style_name, Ref::Unresolved("ISO-25".into()));
    }

    /// Group 42 has no default in the format, and drawings older than R2000
    /// often omit it. Zero is a measurement, so it cannot stand in for one.
    #[test]
    fn a_dimension_without_group_42_states_no_measurement() {
        let d = read_one("  0\nDIMENSION\n  5\n2A\n  8\n0\n 70\n0\n 10\n1.0\n");
        assert_eq!(d.measurement, None);
    }

    /// Group 70 has no default either: without it the file does not say what
    /// the dimension measures.
    #[test]
    fn a_dimension_without_group_70_states_no_kind() {
        let d = read_one("  0\nDIMENSION\n  5\n2A\n  8\n0\n 10\n1.0\n 42\n2.0\n");
        assert_eq!(d.kind, None);
        assert_eq!(d.measurement, Some(2.0));
    }

    /// The three spellings of "show the measurement" are one value, and the
    /// single space that means "show nothing" is another.
    #[test]
    fn the_spellings_of_no_override_fold_into_one_value() {
        let head = "  0\nDIMENSION\n  5\n2A\n  8\n0\n 70\n0\n 10\n0.0\n";
        assert_eq!(read_one(head).text_override, TextOverride::Measured);
        assert_eq!(
            read_one(&format!("{head}  1\n\n")).text_override,
            TextOverride::Measured
        );
        assert_eq!(
            read_one(&format!("{head}  1\n<>\n")).text_override,
            TextOverride::Measured
        );
        assert_eq!(
            read_one(&format!("{head}  1\n \n")).text_override,
            TextOverride::Suppressed
        );
        assert_eq!(
            read_one(&format!("{head}  1\n<> H7\n")).text_override,
            TextOverride::Literal("<> H7".into())
        );
    }

    /// An arc-length dimension is its own entity in the format, and the
    /// group 70 it carries says 5 -- a three-point angular dimension. Going
    /// by the group alone would file it as the wrong thing.
    #[test]
    fn an_arc_length_dimension_is_named_not_flagged() {
        let d = read_one(
            "  0
ARC_DIMENSION
  5
2A
  8
0
  2
*D4
 70
37
             10
1.0
 13
2.0
 14
3.0
 15
4.0
 16
5.0
",
        );
        assert_eq!(d.kind, Some(DimensionKind::ArcLength));
        assert_eq!(d.points.radial.expect("group 15").x, 4.0);
        assert_eq!(d.points.arc.expect("group 16").x, 5.0);
    }

    /// A file may state the subtype with the subclass markers and omit
    /// group 70 entirely -- 18 of the 19 such dimensions in the corpus do.
    /// Reading only group 70 would throw away what those files said.
    #[test]
    fn a_dimension_without_group_70_falls_back_to_its_subclass_markers() {
        let d = read_one(
            "  0
DIMENSION
  5
2A
  8
0
100
AcDbEntity
             100
AcDbDimension
 10
1.0
100
AcDbAlignedDimension
             100
AcDbRotatedDimension
",
        );
        // Both markers are present, as they are on a rotated dimension; the
        // more specific one is the later one.
        assert_eq!(d.kind, Some(DimensionKind::Rotated));
    }

    /// Nothing says it: no group 70, no markers. Drawings old enough have
    /// neither, and then the file really did not say.
    #[test]
    fn a_dimension_with_neither_group_70_nor_markers_states_no_kind() {
        let d = read_one(
            "  0
DIMENSION
  5
2A
  8
0
 10
1.0
",
        );
        assert_eq!(d.kind, None);
    }

    /// The subtype bits are the low three of group 70; the rest are flags.
    #[test]
    fn the_subtype_is_the_low_three_bits_of_group_70() {
        let head = "  0\nDIMENSION\n  5\n2A\n  8\n0\n 10\n0.0\n 70\n";
        for (flag, kind) in [
            (32, DimensionKind::Rotated),
            (33, DimensionKind::Aligned),
            (34, DimensionKind::Angular2Line),
            (35, DimensionKind::Diameter),
            (36, DimensionKind::Radius),
            (37, DimensionKind::Angular3Point),
            (38, DimensionKind::Ordinate),
        ] {
            assert_eq!(
                read_one(&format!("{head}{flag}\n")).kind,
                Some(kind),
                "flag {flag}"
            );
        }
        // 7 is not one of the seven the format defines.
        assert_eq!(read_one(&format!("{head}39\n")).kind, None);
    }
}

/// G5 is the dimension-dense case: the spellings of "show the measurement"
/// no corpus file carries, the space that means "show nothing", a literal
/// that disagrees with the measurement beside it, an arc-length dimension,
/// and a style table that states three variables and stays silent on the
/// rest. It must read back as exactly the model its spec says.
#[test]
fn g5_reads_to_exactly_the_expected_model() {
    let db = read_bytes(include_bytes!("golden/g5.dxf")).expect("g5 reads");
    let expected: CadDatabase = serde_json::from_str(include_str!("golden/g5.expected.json"))
        .expect("the golden model deserializes");
    assert_eq!(db.tables.dim_styles, expected.tables.dim_styles);
    for (got, want) in db.entities.iter().zip(&expected.entities) {
        assert_eq!(got, want);
    }
    assert_eq!(db, expected);
}

/// A group the entity cannot be understood without is reported, not filled.
///
/// Most absent groups have a reading -- a flag word that is not there means
/// no bit is set, an absent colour means ByLayer. Those are silent because
/// there is nothing to report. An attribute's tag is the other kind: it is
/// how a consumer asks for the attribute and how a block definition lines up
/// with its references, so an attribute without one is not an attribute
/// named "" but an entity this reader cannot place. Filling it would let a
/// damaged file read as an intact one.
#[test]
fn a_required_group_that_is_absent_is_reported_rather_than_filled() {
    let tagged = "  0\nSECTION\n  2\nENTITIES\n  0\nATTRIB\n  5\n40\n  8\n0\n  2\nPART\n  1\nX\n 10\n0\n 20\n0\n 40\n2.5\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(tagged).unwrap();
    assert!(
        db.read_diagnostics.is_clean(),
        "an attribute with a tag reports nothing: {:?}",
        db.read_diagnostics.warnings
    );

    let untagged = "  0\nSECTION\n  2\nENTITIES\n  0\nATTRIB\n  5\n41\n  8\n0\n  1\nX\n 10\n0\n 20\n0\n 40\n2.5\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(untagged).unwrap();
    // The entity is still read -- reporting is not refusing.
    assert_eq!(db.entities.len(), 1);
    assert_eq!(db.read_diagnostics.warnings.len(), 1);
    // The whole message, not a prefix: the other reader of this model says
    // the same words, so a drawing read through either says the same thing.
    assert_eq!(
        db.read_diagnostics.warnings[0],
        "MISSING_REQUIRED_GROUP: ATTRIB carries no tag (group 2); the entity is kept with an empty one"
    );
}

fn only_leader(db: &CadDatabase) -> &LeaderEntity {
    db.entities
        .iter()
        .find_map(|e| match e {
            Entity::Leader(l) => Some(l),
            _ => None,
        })
        .expect("a LEADER")
}

/// The entity a leader names may come later in the file than the leader:
/// the reference resolves once every entity has been read.
#[test]
fn a_leader_reference_resolves_to_an_entity_written_after_it() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nLEADER\n  5\n40\n  8\n0\n 10\n0\n 20\n0\n 10\n1\n 20\n1\n340\n41\n  0\nLINE\n  5\n41\n  8\n0\n 10\n0\n 20\n0\n 11\n1\n 21\n1\n  0\nENDSEC\n  0\nEOF\n";
    let leader = only_leader(&read_str(text).unwrap()).clone();
    assert_eq!(leader.annotation_id, Ref::Resolved(EntityId::new(0x41)));
}

/// A handle nothing in the drawing carries is a different fact from no
/// handle at all, and the null handle is the file naming nothing.
#[test]
fn a_leader_reference_to_no_entity_keeps_its_handle_and_a_null_one_is_absent() {
    let dangling = "  0\nSECTION\n  2\nENTITIES\n  0\nLEADER\n  5\n40\n  8\n0\n 10\n0\n 20\n0\n 10\n1\n 20\n1\n340\n9f\n  0\nENDSEC\n  0\nEOF\n";
    assert_eq!(
        only_leader(&read_str(dangling).unwrap()).annotation_id,
        Ref::Unresolved("9F".into())
    );
    let null = "  0\nSECTION\n  2\nENTITIES\n  0\nLEADER\n  5\n40\n  8\n0\n 10\n0\n 20\n0\n 10\n1\n 20\n1\n340\n0\n  0\nENDSEC\n  0\nEOF\n";
    assert_eq!(
        only_leader(&read_str(null).unwrap()).annotation_id,
        Ref::Absent
    );
}

/// Group 71 may be omitted from a text drawing, and then the file has not
/// said whether the leader draws an arrowhead.
#[test]
fn a_leader_whose_file_omits_the_arrowhead_flag_does_not_say_either_way() {
    let omitted = "  0\nSECTION\n  2\nENTITIES\n  0\nLEADER\n  5\n40\n  8\n0\n 10\n0\n 20\n0\n 10\n1\n 20\n1\n  0\nENDSEC\n  0\nEOF\n";
    assert_eq!(only_leader(&read_str(omitted).unwrap()).has_arrowhead, None);
    let stated = "  0\nSECTION\n  2\nENTITIES\n  0\nLEADER\n  5\n40\n  8\n0\n 71\n0\n 10\n0\n 20\n0\n 10\n1\n 20\n1\n  0\nENDSEC\n  0\nEOF\n";
    assert_eq!(
        only_leader(&read_str(stated).unwrap()).has_arrowhead,
        Some(false)
    );
}

/// A circle without a radius is not a circle of radius 0: the file is
/// damaged, and the reader says what it filled in.
#[test]
fn a_size_the_file_does_not_state_is_reported_not_passed_off_as_zero() {
    let circle = "  0\nSECTION\n  2\nENTITIES\n  0\nCIRCLE\n  5\n50\n  8\n0\n 10\n1\n 20\n1\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(circle).unwrap();
    assert_eq!(db.entities.len(), 1, "reporting is not refusing");
    assert_eq!(
        db.read_diagnostics.warnings,
        ["MISSING_REQUIRED_GROUP: CIRCLE carries no radius (group 40); the entity is kept with 0"]
    );

    let arc = "  0\nSECTION\n  2\nENTITIES\n  0\nARC\n  5\n51\n  8\n0\n 10\n0\n 20\n0\n 40\n2\n  0\nENDSEC\n  0\nEOF\n";
    assert_eq!(
        read_str(arc).unwrap().read_diagnostics.warnings,
        [
            "MISSING_REQUIRED_GROUP: ARC carries no start angle (group 50); the entity is kept with 0",
            "MISSING_REQUIRED_GROUP: ARC carries no end angle (group 51); the entity is kept with 0",
        ]
    );
}

/// The other side of the same line: a rotation the file leaves out is a
/// rotation of zero, which writers omit, and reporting it would call an
/// intact file damaged.
#[test]
fn an_omitted_rotation_is_read_not_reported() {
    let text = "  0\nSECTION\n  2\nENTITIES\n  0\nTEXT\n  5\n52\n  8\n0\n 10\n0\n 20\n0\n 40\n2.5\n  1\nA\n  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    assert!(
        db.read_diagnostics.is_clean(),
        "{:?}",
        db.read_diagnostics.warnings
    );
}
