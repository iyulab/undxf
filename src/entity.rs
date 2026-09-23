//! One entity from its pairs. The pairs of an entity are everything after
//! its `0` line up to the next `0` line; which of them mean what is the DXF
//! reference's table for that type.

use crate::pairs::{Pair, ReadError};
use uncad_model::model::{
    ArcEntity, AttdefEntity, AttribEntity, CircleEntity, Confidence, DimensionEntity,
    DimensionKind, DimensionPoints, EllipseEntity, Entity, EntityCommon, EntityId, Face3DEntity,
    HatchEntity, InsertEntity, LeaderAnnotation, LeaderEntity, LeaderPath, LineEntity,
    LwPolylineEntity, MTextAttachment, MTextEntity, Origin, Point2D, Point3D, PointEntity,
    PolylineVertex, RayEntity, Ref, SolidEntity, SplineEntity, TextEntity, TextHorizontalAlignment,
    TextOverride, TextVerticalAlignment, ViewportEntity,
};

/// Where the IDs of handle-less entities live: above every possible handle
/// value, so they can never collide with a handle-derived ID. The same rule
/// as the other reader of this model, so both name a handle-less entity the
/// same way.
const HANDLELESS_ID_BASE: u64 = 1 << 63;

/// Which space an entity of the ENTITIES section belongs to (DXF 67).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Model,
    Paper,
}

/// An entity as read, before its references are resolved against the
/// tables: layer and block names are carried as the file wrote them.
pub struct Read {
    pub entity: Entity,
    pub space: Space,
    /// `true` for an INSERT whose attributes follow (DXF 66).
    pub attribs_follow: bool,
    /// What this reader had to substitute for something the file did not
    /// say, where the substitute is not a reading the format supports.
    ///
    /// Most absent groups have an answer: a flag word that is not there
    /// means no bit is set, an absent colour means ByLayer, an absent scale
    /// means one. Those are readings, and they are silent because there is
    /// nothing to report. What lands here is the other kind -- a group the
    /// entity cannot be understood without, where filling a value would let
    /// a damaged file read as an intact one.
    pub warnings: Vec<String>,
}

fn number(p: &Pair<'_>) -> Result<f64, ReadError> {
    p.value.trim().parse().map_err(|_| ReadError::BadNumber {
        line: p.line,
        code: p.code,
        text: p.value.to_string(),
    })
}

fn integer(p: &Pair<'_>) -> Result<i64, ReadError> {
    p.value.trim().parse().map_err(|_| ReadError::BadNumber {
        line: p.line,
        code: p.code,
        text: p.value.to_string(),
    })
}

/// The first value of group `code`, as a number.
fn num(pairs: &[Pair<'_>], code: i32) -> Result<Option<f64>, ReadError> {
    pairs
        .iter()
        .find(|p| p.code == code)
        .map(number)
        .transpose()
}

pub(crate) fn num_or(pairs: &[Pair<'_>], code: i32, default: f64) -> Result<f64, ReadError> {
    Ok(num(pairs, code)?.unwrap_or(default))
}

fn int(pairs: &[Pair<'_>], code: i32) -> Result<Option<i64>, ReadError> {
    pairs
        .iter()
        .find(|p| p.code == code)
        .map(integer)
        .transpose()
}

fn text<'a>(pairs: &[Pair<'a>], code: i32) -> Option<&'a str> {
    pairs.iter().find(|p| p.code == code).map(|p| p.value)
}

/// The 10/20/30 point of a record (a VERTEX's position), for the reader.
pub fn point3_of(pairs: &[Pair<'_>]) -> Result<Point3D, ReadError> {
    point3(pairs, 10)
}

fn point3(pairs: &[Pair<'_>], x: i32) -> Result<Point3D, ReadError> {
    Ok(Point3D {
        x: num_or(pairs, x, 0.0)?,
        y: num_or(pairs, x + 10, 0.0)?,
        z: num_or(pairs, x + 20, 0.0)?,
    })
}

fn point2(pairs: &[Pair<'_>], x: i32) -> Result<Point2D, ReadError> {
    Ok(Point2D {
        x: num_or(pairs, x, 0.0)?,
        y: num_or(pairs, x + 10, 0.0)?,
    })
}

/// The extrusion direction (DXF 210), written only when it is not the
/// default Z axis.
pub(crate) fn extrusion(pairs: &[Pair<'_>]) -> Result<Point3D, ReadError> {
    Ok(optional_point3(pairs, 210)?.unwrap_or(Point3D {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    }))
}

fn radians(pairs: &[Pair<'_>], code: i32) -> Result<f64, ReadError> {
    Ok(num_or(pairs, code, 0.0)?.to_radians())
}

/// The `common` block of an entity. `ordinal` numbers handle-less entities.
fn common(pairs: &[Pair<'_>], ordinal: u64) -> Result<EntityCommon, ReadError> {
    let (id, source_handle) = match text(pairs, 5) {
        Some(h) => {
            let line = pairs.iter().find(|p| p.code == 5).map_or(0, |p| p.line);
            let value = u64::from_str_radix(h.trim(), 16).map_err(|_| ReadError::BadNumber {
                line,
                code: 5,
                text: h.to_string(),
            })?;
            (
                EntityId::new(value),
                Ref::Resolved(h.trim().to_ascii_uppercase()),
            )
        }
        None => (EntityId::new(HANDLELESS_ID_BASE | ordinal), Ref::Absent),
    };
    // The layer is carried by name here and resolved against the LAYER
    // table by the reader once every table is in.
    let layer = match text(pairs, 8) {
        Some(name) => Ref::Unresolved(name.to_string()),
        None => Ref::Absent,
    };
    let color_index = int(pairs, 62)?.map_or(256, |c| c as i16);
    let true_color = int(pairs, 420)?.map(|c| (c as u32) & 0xff_ffff);
    Ok(EntityCommon {
        id,
        origin: Origin::Vector,
        confidence: Confidence::High,
        source_handle,
        layer,
        color_index,
        true_color,
        // Written only when set: no group 60 is a visible entity.
        invisible: int(pairs, 60)? == Some(1),
    })
}

/// Groups an entity cannot be understood without: `(type, group, what the
/// group is, what this reader keeps in its place)`.
///
/// Two conditions put a group here, and both must hold. The DXF reference
/// lists it without "optional" -- its convention for a group that always
/// appears -- and no drawing this reader has been measured against leaves
/// it out. The second condition is not a formality: writers do omit some
/// groups the reference lists as always present, when their value is zero.
/// Those are rotations and flags (a text's rotation away from its default
/// direction, a leader's arrowhead flag), and reporting them would call an
/// intact file damaged, so they are read or carried as unstated instead and
/// are not listed. What is listed are the values an entity's size and shape
/// rest on, which were never seen missing.
///
/// An attribute's tag is here for a different reason: it is how a consumer
/// asks for the attribute and how a block definition and its references
/// line up, so an attribute without one is an entity nothing can place.
const REQUIRED_GROUPS: &[(&str, i32, &str, &str)] = &[
    ("ATTRIB", 2, "tag", "an empty one"),
    ("ATTDEF", 2, "tag", "an empty one"),
    ("ATTRIB", 40, "text height", "0"),
    ("ATTDEF", 40, "text height", "0"),
    ("TEXT", 40, "text height", "0"),
    ("CIRCLE", 40, "radius", "0"),
    ("ARC", 40, "radius", "0"),
    ("ARC", 50, "start angle", "0"),
    ("ARC", 51, "end angle", "0"),
    ("ELLIPSE", 11, "major axis", "a zero-length one"),
    ("ELLIPSE", 40, "axis ratio", "0"),
    ("ELLIPSE", 41, "start parameter", "0"),
    ("ELLIPSE", 42, "end parameter", "a full turn"),
    ("SPLINE", 71, "degree", "0"),
    ("MTEXT", 10, "insertion point", "the origin"),
    ("MTEXT", 40, "text height", "0"),
    ("DIMENSION", 11, "text position", "the origin"),
    ("ARC_DIMENSION", 11, "text position", "the origin"),
    ("POINT", 10, "position", "the origin"),
    ("SOLID", 10, "first corner", "the origin"),
    ("SOLID", 11, "second corner", "the origin"),
    ("SOLID", 12, "third corner", "the origin"),
    ("TRACE", 10, "first corner", "the origin"),
    ("TRACE", 11, "second corner", "the origin"),
    ("TRACE", 12, "third corner", "the origin"),
    ("3DFACE", 10, "first corner", "the origin"),
    ("3DFACE", 11, "second corner", "the origin"),
    ("3DFACE", 12, "third corner", "the origin"),
    ("RAY", 10, "base point", "the origin"),
    ("RAY", 11, "direction", "the zero vector"),
    ("XLINE", 10, "base point", "the origin"),
    ("XLINE", 11, "direction", "the zero vector"),
    ("VIEWPORT", 10, "center", "the origin"),
    ("VIEWPORT", 40, "width", "0"),
    ("VIEWPORT", 41, "height", "0"),
];

/// What this reader had to substitute for a required group of `type_name`
/// (see [`REQUIRED_GROUPS`]), one line per group, in the table's order.
/// Reporting is not refusing: the entity is kept, and the reader says what
/// it made up.
fn missing_required_groups(type_name: &str, pairs: &[Pair<'_>]) -> Vec<String> {
    REQUIRED_GROUPS
        .iter()
        .filter(|(t, code, ..)| *t == type_name && text(pairs, *code).is_none_or(str::is_empty))
        .map(|(t, code, what, kept)| {
            format!(
                "MISSING_REQUIRED_GROUP: {t} carries no {what} (group {code}); the entity is kept with {kept}"
            )
        })
        .collect()
}

/// Builds the entity `type_name` from its pairs.
pub fn read(type_name: &str, pairs: &[Pair<'_>], ordinal: u64) -> Result<Read, ReadError> {
    let common = common(pairs, ordinal)?;
    let space = match int(pairs, 67)? {
        Some(1) => Space::Paper,
        _ => Space::Model,
    };
    let mut attribs_follow = false;
    let mut warnings = missing_required_groups(type_name, pairs);
    let entity = match type_name {
        "LINE" => Entity::Line(LineEntity {
            common,
            start_point: point3(pairs, 10)?,
            end_point: point3(pairs, 11)?,
        }),
        "CIRCLE" => Entity::Circle(CircleEntity {
            common,
            center: point3(pairs, 10)?,
            radius: num_or(pairs, 40, 0.0)?,
            extrusion: extrusion(pairs)?,
        }),
        "ARC" => Entity::Arc(ArcEntity {
            common,
            center: point3(pairs, 10)?,
            radius: num_or(pairs, 40, 0.0)?,
            start_angle: radians(pairs, 50)?,
            end_angle: radians(pairs, 51)?,
            extrusion: extrusion(pairs)?,
        }),
        "LWPOLYLINE" => {
            // Vertices are the 10/20 pairs in order; a 10 opens a vertex,
            // and a 42 that follows it (before the next 10) is that vertex's
            // bulge. An absent 42 is a straight segment.
            let mut vertices: Vec<PolylineVertex> = Vec::new();
            let mut i = 0;
            while i < pairs.len() {
                match pairs[i].code {
                    10 => {
                        let x = number(&pairs[i])?;
                        let y = match pairs.get(i + 1) {
                            Some(p) if p.code == 20 => {
                                i += 1;
                                number(p)?
                            }
                            _ => 0.0,
                        };
                        vertices.push(PolylineVertex::straight(Point2D { x, y }));
                    }
                    42 => {
                        if let Some(last) = vertices.last_mut() {
                            last.bulge = number(&pairs[i])?;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let flags = int(pairs, 70)?.unwrap_or(0);
            Entity::LwPolyline(LwPolylineEntity {
                common,
                vertices,
                closed: flags & 1 == 1,
                elevation: num_or(pairs, 38, 0.0)?,
                extrusion: extrusion(pairs)?,
            })
        }
        "POINT" => Entity::Point(PointEntity {
            common,
            position: point3(pairs, 10)?,
        }),
        "SOLID" | "TRACE" => {
            let corner3 = point2(pairs, 12)?;
            // A three-cornered one leaves the fourth corner out, or writes
            // it equal to the third; the reference says the two are the
            // same.
            let solid = SolidEntity {
                common,
                corner1: point2(pairs, 10)?,
                corner2: point2(pairs, 11)?,
                corner3,
                corner4: match text(pairs, 13) {
                    Some(_) => point2(pairs, 13)?,
                    None => corner3,
                },
                // The corners share one z in their own plane; the first
                // corner's is the one every writer states.
                elevation: num_or(pairs, 30, 0.0)?,
                extrusion: extrusion(pairs)?,
            };
            if type_name == "SOLID" {
                Entity::Solid(solid)
            } else {
                Entity::Trace(solid)
            }
        }
        "3DFACE" => {
            let corner3 = point3(pairs, 12)?;
            Entity::Face3D(Face3DEntity {
                common,
                corner1: point3(pairs, 10)?,
                corner2: point3(pairs, 11)?,
                corner3,
                corner4: match text(pairs, 13) {
                    Some(_) => point3(pairs, 13)?,
                    None => corner3,
                },
                invisible_edges: Face3DEntity::invisible_edges_from_bits(
                    int(pairs, 70)?.unwrap_or(0) as u32,
                ),
            })
        }
        "TEXT" => {
            let (horizontal_alignment, vertical_alignment, alignment_point) =
                placement(type_name, pairs, 73, &mut warnings)?;
            Entity::Text(TextEntity {
                common,
                start_point: point2(pairs, 10)?,
                text_height: num_or(pairs, 40, 0.0)?,
                text: text(pairs, 1).unwrap_or("").to_string(),
                rotation: radians(pairs, 50)?,
                horizontal_alignment,
                vertical_alignment,
                alignment_point,
                // A fraction of the normal width: absent is 1.
                width_factor: num_or(pairs, 41, 1.0)?,
            })
        }
        "ATTRIB" => {
            let (horizontal_alignment, vertical_alignment, alignment_point) =
                placement(type_name, pairs, 74, &mut warnings)?;
            Entity::Attrib(AttribEntity {
                common,
                start_point: point2(pairs, 10)?,
                text_height: num_or(pairs, 40, 0.0)?,
                tag: text(pairs, 2).unwrap_or("").to_string(),
                text: text(pairs, 1).unwrap_or("").to_string(),
                rotation: radians(pairs, 50)?,
                horizontal_alignment,
                vertical_alignment,
                alignment_point,
                width_factor: num_or(pairs, 41, 1.0)?,
            })
        }
        "ATTDEF" => {
            let (horizontal_alignment, vertical_alignment, alignment_point) =
                placement(type_name, pairs, 74, &mut warnings)?;
            Entity::Attdef(AttdefEntity {
                common,
                start_point: point2(pairs, 10)?,
                text_height: num_or(pairs, 40, 0.0)?,
                tag: text(pairs, 2).unwrap_or("").to_string(),
                default_value: text(pairs, 1).unwrap_or("").to_string(),
                rotation: radians(pairs, 50)?,
                horizontal_alignment,
                vertical_alignment,
                alignment_point,
                width_factor: num_or(pairs, 41, 1.0)?,
            })
        }
        "INSERT" => {
            attribs_follow = int(pairs, 66)? == Some(1);
            Entity::Insert(InsertEntity {
                common,
                block_name: name_ref(text(pairs, 2)),
                insertion_point: point3(pairs, 10)?,
                scale: Point3D {
                    x: num_or(pairs, 41, 1.0)?,
                    y: num_or(pairs, 42, 1.0)?,
                    z: num_or(pairs, 43, 1.0)?,
                },
                rotation: radians(pairs, 50)?,
                attribs: Vec::new(),
            })
        }
        "DIMENSION" | "ARC_DIMENSION" => Entity::Dimension(DimensionEntity {
            common,
            block_name: name_ref(text(pairs, 2)),
            kind: dimension_kind(type_name, int(pairs, 70)?, pairs),
            measurement: num(pairs, 42)?,
            text_override: text_override(text(pairs, 1)),
            definition_point: Some(point3(pairs, 10)?),
            text_midpoint: point2(pairs, 11)?,
            points: DimensionPoints {
                extension1: optional_point3(pairs, 13)?,
                extension2: optional_point3(pairs, 14)?,
                radial: optional_point3(pairs, 15)?,
                arc: optional_point3(pairs, 16)?,
            },
            rotation: radians(pairs, 50)?,
            text_rotation: radians(pairs, 53)?,
            style_name: name_ref(text(pairs, 3)),
        }),
        "LEADER" => Entity::Leader(LeaderEntity {
            common,
            vertices: repeated_point3(pairs, 10)?,
            // Group 71 is a flag: drawn or not. A text drawing may omit it,
            // and a value outside the two is not one this reader can read.
            has_arrowhead: match int(pairs, 71)? {
                Some(0) => Some(false),
                Some(1) => Some(true),
                _ => None,
            },
            // 0 straight, 1 spline. The format does not say what an absent
            // group means here, so an absent one is nothing.
            path_type: match int(pairs, 72)? {
                Some(0) => Some(LeaderPath::Straight),
                Some(1) => Some(LeaderPath::Spline),
                _ => None,
            },
            // 0 text, 1 tolerance, 2 insert, 3 none -- and 3 is the value the
            // format falls back to, so anything else reads as "nothing".
            annotation: match int(pairs, 73)? {
                Some(0) => LeaderAnnotation::MText,
                Some(1) => LeaderAnnotation::Tolerance,
                Some(2) => LeaderAnnotation::Insert,
                _ => LeaderAnnotation::Nothing,
            },
            annotation_id: handle_ref(pairs, 340),
            style_name: name_ref(text(pairs, 3)),
        }),
        "ELLIPSE" => Entity::Ellipse(EllipseEntity {
            common,
            center: point3(pairs, 10)?,
            major_axis_endpoint: point3(pairs, 11)?,
            axis_ratio: num_or(pairs, 40, 0.0)?,
            // Parameters of the ellipse, already in radians -- not the
            // degrees ARC and TEXT write.
            start_angle: num_or(pairs, 41, 0.0)?,
            end_angle: num_or(pairs, 42, std::f64::consts::TAU)?,
            extrusion: extrusion(pairs)?,
        }),
        "MTEXT" => Entity::MText(MTextEntity {
            common,
            insertion_point: point3(pairs, 10)?,
            text: mtext_string(pairs),
            text_height: num_or(pairs, 40, 0.0)?,
            // The rotation is written as the text's X-axis direction (11),
            // or -- by older writers -- as an angle (50); neither means
            // the text is not turned.
            rotation: match optional_point3(pairs, 11)? {
                Some(d) => d.y.atan2(d.x),
                None => radians(pairs, 50)?,
            },
            // A fraction of the default spacing: absent is 1.
            line_spacing_factor: num_or(pairs, 44, 1.0)?,
            // No box to wrap in: absent is 0.
            reference_width: num_or(pairs, 41, 0.0)?,
            attachment: match int(pairs, 71)? {
                Some(1) => Some(MTextAttachment::TopLeft),
                Some(2) => Some(MTextAttachment::TopCenter),
                Some(3) => Some(MTextAttachment::TopRight),
                Some(4) => Some(MTextAttachment::MiddleLeft),
                Some(5) => Some(MTextAttachment::MiddleCenter),
                Some(6) => Some(MTextAttachment::MiddleRight),
                Some(7) => Some(MTextAttachment::BottomLeft),
                Some(8) => Some(MTextAttachment::BottomCenter),
                Some(9) => Some(MTextAttachment::BottomRight),
                _ => None,
            },
        }),
        "SPLINE" => {
            // Bits 1 closed, 2 periodic. No group 70 is a record that does
            // not say, which is not the same as saying no.
            let flags = int(pairs, 70)?;
            Entity::Spline(SplineEntity {
                common,
                degree: u32::try_from(int(pairs, 71)?.unwrap_or(0)).unwrap_or(0),
                closed: flags.map(|f| f & 1 != 0),
                periodic: flags.map(|f| f & 2 != 0),
                knots: repeated_number(pairs, 40)?,
                // Written for every control point or for none; none means
                // every weight is 1.
                weights: repeated_number(pairs, 41)?,
                fit_points: repeated_point3(pairs, 11)?,
                control_points: repeated_point3(pairs, 10)?,
            })
        }
        "RAY" => Entity::Ray(RayEntity {
            common,
            point: point3(pairs, 10)?,
            vector: point3(pairs, 11)?,
        }),
        "XLINE" => Entity::XLine(RayEntity {
            common,
            point: point3(pairs, 10)?,
            vector: point3(pairs, 11)?,
        }),
        // A paper-space viewport's frame: where it sits on the sheet and its
        // size there. What it looks at in model space is not carried.
        "VIEWPORT" => Entity::Viewport(ViewportEntity {
            common,
            center: point3(pairs, 10)?,
            width: num_or(pairs, 40, 0.0)?,
            height: num_or(pairs, 41, 0.0)?,
        }),
        "HATCH" => {
            let hatch = crate::hatch::read(pairs, &mut warnings)?;
            Entity::Hatch(HatchEntity {
                common,
                boundary_paths: hatch.boundary_paths,
                solid_fill: hatch.solid_fill,
                gradient: hatch.gradient,
                pattern_lines: hatch.pattern_lines,
            })
        }
        other => Entity::Unknown {
            common,
            type_name: other.to_string(),
        },
    };
    Ok(Read {
        entity,
        space,
        attribs_follow,
        warnings,
    })
}

/// What the dimension measures, from the three places the format states it,
/// in the order the format means them.
///
/// An arc-length dimension is its own entity and still carries a group 70
/// saying 5 (a three-point angular one), so the entity name comes first.
/// Then group 70's low three bits. Then the subclass markers (group 100),
/// which a file may state while omitting group 70 entirely -- 18 of the 19
/// such dimensions in the corpus do exactly that, and reading only group 70
/// would throw away what those files said. `None` means all three were
/// silent, which happens in drawings old enough to have no markers either.
fn dimension_kind(type_name: &str, flag: Option<i64>, pairs: &[Pair<'_>]) -> Option<DimensionKind> {
    if type_name == "ARC_DIMENSION" {
        return Some(DimensionKind::ArcLength);
    }
    if let Some(flag) = flag {
        if let Some(kind) = kind_of_flag(flag) {
            return Some(kind);
        }
    }
    kind_of_markers(pairs)
}

/// The low three bits of DXF 70. A value outside the seven the format
/// defines is not a subtype this reader knows, and says so with `None`
/// rather than picking the nearest one.
fn kind_of_flag(flag: i64) -> Option<DimensionKind> {
    Some(match flag & 7 {
        0 => DimensionKind::Rotated,
        1 => DimensionKind::Aligned,
        2 => DimensionKind::Angular2Line,
        3 => DimensionKind::Diameter,
        4 => DimensionKind::Radius,
        5 => DimensionKind::Angular3Point,
        6 => DimensionKind::Ordinate,
        _ => return None,
    })
}

/// The last subclass marker (group 100) that names a dimension subtype. A
/// rotated dimension carries both `AcDbAlignedDimension` and
/// `AcDbRotatedDimension`, in that order, so the most specific one is last.
fn kind_of_markers(pairs: &[Pair<'_>]) -> Option<DimensionKind> {
    let mut kind = None;
    for p in pairs.iter().filter(|p| p.code == 100) {
        kind = match p.value {
            "AcDbRotatedDimension" => Some(DimensionKind::Rotated),
            "AcDbAlignedDimension" => Some(DimensionKind::Aligned),
            "AcDb2LineAngularDimension" => Some(DimensionKind::Angular2Line),
            "AcDbDiametricDimension" => Some(DimensionKind::Diameter),
            "AcDbRadialDimension" => Some(DimensionKind::Radius),
            "AcDb3PointAngularDimension" => Some(DimensionKind::Angular3Point),
            "AcDbOrdinateDimension" => Some(DimensionKind::Ordinate),
            "AcDbArcDimension" => Some(DimensionKind::ArcLength),
            _ => continue,
        };
    }
    kind
}

/// DXF 1 folded to one value per meaning: the group absent, empty, or `<>`
/// all say "show the measurement"; a single space says "show nothing".
fn text_override(value: Option<&str>) -> TextOverride {
    match value {
        None | Some("") | Some("<>") => TextOverride::Measured,
        Some(" ") => TextOverride::Suppressed,
        Some(other) => TextOverride::Literal(other.to_string()),
    }
}

/// Where a TEXT, ATTRIB or ATTDEF is aligned: its horizontal alignment (72),
/// its vertical alignment (`vertical` -- 73 for a TEXT, 74 for an attribute,
/// whose 73 is its field length) and its alignment point (11). An absent
/// alignment group is the default. A value outside the format's range is
/// reported, and read as the default rather than refused. The point is kept
/// only for an alignment other than left and baseline -- the only case in
/// which the format writes it.
fn placement(
    type_name: &str,
    pairs: &[Pair<'_>],
    vertical: i32,
    warnings: &mut Vec<String>,
) -> Result<
    (
        TextHorizontalAlignment,
        TextVerticalAlignment,
        Option<Point2D>,
    ),
    ReadError,
> {
    let h = match int(pairs, 72)? {
        None | Some(0) => TextHorizontalAlignment::Left,
        Some(1) => TextHorizontalAlignment::Center,
        Some(2) => TextHorizontalAlignment::Right,
        Some(3) => TextHorizontalAlignment::Aligned,
        Some(4) => TextHorizontalAlignment::Middle,
        Some(5) => TextHorizontalAlignment::Fit,
        Some(other) => {
            warnings.push(format!(
                "TEXT_ALIGNMENT: a {type_name} states horizontal alignment {other} (group 72), outside 0 to 5; it is read as left"
            ));
            TextHorizontalAlignment::Left
        }
    };
    let v = match int(pairs, vertical)? {
        None | Some(0) => TextVerticalAlignment::Baseline,
        Some(1) => TextVerticalAlignment::Bottom,
        Some(2) => TextVerticalAlignment::Middle,
        Some(3) => TextVerticalAlignment::Top,
        Some(other) => {
            warnings.push(format!(
                "TEXT_ALIGNMENT: a {type_name} states vertical alignment {other} (group {vertical}), outside 0 to 3; it is read as baseline"
            ));
            TextVerticalAlignment::Baseline
        }
    };
    let aligned = (h, v)
        != (
            TextHorizontalAlignment::Left,
            TextVerticalAlignment::Baseline,
        );
    let point = if aligned {
        optional_point2(pairs, 11)?
    } else {
        None
    };
    Ok((h, v, point))
}

/// A 2D point the file may leave out: absent when its x group is.
fn optional_point2(pairs: &[Pair<'_>], x: i32) -> Result<Option<Point2D>, ReadError> {
    if num(pairs, x)?.is_none() {
        return Ok(None);
    }
    Ok(Some(point2(pairs, x)?))
}

/// A point the file carries only for some dimension subtypes. Its absence is
/// the x group's absence: a subtype that does not use the point writes none
/// of its three groups.
fn optional_point3(pairs: &[Pair<'_>], x: i32) -> Result<Option<Point3D>, ReadError> {
    if num(pairs, x)?.is_none() {
        return Ok(None);
    }
    Ok(Some(point3(pairs, x)?))
}

/// An MTEXT's text: a long one is written as 250-character pieces in
/// group 3, in order, and ends with its last piece in group 1.
fn mtext_string(pairs: &[Pair<'_>]) -> String {
    let mut out: String = pairs
        .iter()
        .filter(|p| p.code == 3)
        .map(|p| p.value)
        .collect();
    out.push_str(text(pairs, 1).unwrap_or(""));
    out
}

/// Every value of a group a record repeats (a spline's knots, its weights),
/// in the order the file writes them.
fn repeated_number(pairs: &[Pair<'_>], code: i32) -> Result<Vec<f64>, ReadError> {
    pairs
        .iter()
        .filter(|p| p.code == code)
        .map(number)
        .collect()
}

/// Every 10/20/30 triple in a record, in the order the file writes them.
///
/// Unlike [`point3`], which finds one point by its group code, a leader
/// writes its vertices as repeats of the same three codes -- so they are
/// read positionally: a 10 opens a vertex and the 20/30 that follow it
/// belong to it. A missing 20 or 30 is zero, the same reading the
/// single-point helper gives.
fn repeated_point3(pairs: &[Pair<'_>], x: i32) -> Result<Vec<Point3D>, ReadError> {
    let mut points = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].code != x {
            i += 1;
            continue;
        }
        let mut point = Point3D {
            x: number(&pairs[i])?,
            y: 0.0,
            z: 0.0,
        };
        if let Some(p) = pairs.get(i + 1).filter(|p| p.code == x + 10) {
            point.y = number(p)?;
            i += 1;
            if let Some(p) = pairs.get(i + 1).filter(|p| p.code == x + 20) {
                point.z = number(p)?;
                i += 1;
            }
        }
        points.push(point);
        i += 1;
    }
    Ok(points)
}

/// A reference to another entity, by the handle the file writes, carried
/// unresolved until the reader has seen every entity: the target may come
/// later in the file than the entity that names it.
///
/// No group, or the null handle `0`, is a file that names nothing. Anything
/// else is kept as the file wrote it (upper-case hex, the form an entity's
/// own handle takes) -- including text that is not a hexadecimal number,
/// which is a reference nothing can answer to rather than no reference.
fn handle_ref(pairs: &[Pair<'_>], code: i32) -> Ref<EntityId> {
    let Some(handle) = text(pairs, code).map(str::trim) else {
        return Ref::Absent;
    };
    match u64::from_str_radix(handle, 16) {
        Ok(0) => Ref::Absent,
        _ if handle.is_empty() => Ref::Absent,
        _ => Ref::Unresolved(handle.to_ascii_uppercase()),
    }
}

/// A block name as the file wrote it, to be resolved against the BLOCKS
/// section by the reader. No name at all is [`Ref::Absent`].
fn name_ref(name: Option<&str>) -> Ref<String> {
    match name {
        Some(n) if !n.is_empty() => Ref::Unresolved(n.to_string()),
        _ => Ref::Absent,
    }
}
