//! One entity from its pairs. The pairs of an entity are everything after
//! its `0` line up to the next `0` line; which of them mean what is the DXF
//! reference's table for that type.

use crate::decode::string;
use crate::pairs::{Pair, ReadError};
use uncad_model::model::{
    ArcEntity, AttdefEntity, AttribEntity, AttributeFlags, CircleEntity, Confidence,
    DimensionEntity, DimensionKind, DimensionPoints, EllipseEntity, Entity, EntityCommon, EntityId,
    EntityLinetype, Face3DEntity, HatchEntity, HorizontalJustification, ImageEntity, InsertEntity,
    LeaderAnnotation, LeaderEntity, LeaderPath, LightEntity, LightType, LineEntity,
    LwPolylineEntity, MLineEntity, MLineVertex, MTextAttachment, MTextEntity, MultiLeaderEntity,
    OrdinateAxis, Origin, Point2D, Point3D, PointEntity, PolylineVertex, RayEntity, Ref,
    SolidEntity, SplineEntity, TextEntity, TextOverride, ToleranceEntity, VerticalJustification,
    ViewportEntity, ViewportView, WipeoutEntity,
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
        Some(name) => Ref::Unresolved(string(name)),
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
        linetype: linetype(text(pairs, 6)),
        linetype_scale: num_or(pairs, 48, 1.0)?,
        // Carried as stated; an absent group becomes BYLAYER once the
        // drawing's version is known to have the property (`Reader::finish`).
        lineweight: int(pairs, 370)?.map(|w| w as i16),
        transparency: int(pairs, 440)?.map(|t| t as u32),
    })
}

/// An entity's linetype (DXF 6): absent, or the pseudo-names BYLAYER and
/// BYBLOCK in any case, are not a table entry; any other name is carried
/// and resolved against the LTYPE table with the rest of the drawing.
fn linetype(name: Option<&str>) -> EntityLinetype {
    match name.map(str::trim) {
        None => EntityLinetype::ByLayer,
        Some(n) if n.eq_ignore_ascii_case("BYLAYER") => EntityLinetype::ByLayer,
        Some(n) if n.eq_ignore_ascii_case("BYBLOCK") => EntityLinetype::ByBlock,
        Some(n) => EntityLinetype::Named(Ref::Unresolved(string(n))),
    }
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
    ("TOLERANCE", 10, "insertion point", "the origin"),
    ("WIPEOUT", 10, "insertion point", "the origin"),
    ("IMAGE", 10, "insertion point", "the origin"),
    ("IMAGE", 11, "pixel width vector", "the zero vector"),
    ("IMAGE", 12, "pixel height vector", "the zero vector"),
    ("IMAGE", 13, "size in pixels", "0 by 0"),
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

/// The groups every entity carries: everything before the first subclass
/// marker after `AcDbEntity`. A type's own subclass may reuse their codes --
/// a section object's group 62 is its indicator colour, not the entity's --
/// so they are not looked for past it. A record without subclass markers
/// (before R13) is all common part and type part at once.
fn common_part<'p, 'a>(pairs: &'p [Pair<'a>]) -> &'p [Pair<'a>] {
    let end = pairs
        .iter()
        .position(|p| p.code == 100 && p.value.trim() != "AcDbEntity")
        .unwrap_or(pairs.len());
    &pairs[..end]
}

/// The entity's own groups: everything before an embedded object. A
/// multi-line attribute, for one, carries an MTEXT after a group 101 marker,
/// and that object writes its own insertion point, height and text under
/// the same codes as the attribute -- they are not the attribute's.
fn own_part<'p, 'a>(pairs: &'p [Pair<'a>]) -> &'p [Pair<'a>] {
    let end = pairs
        .iter()
        .position(|p| p.code == 101)
        .unwrap_or(pairs.len());
    &pairs[..end]
}

/// Builds the entity `type_name` from its pairs.
pub fn read(type_name: &str, pairs: &[Pair<'_>], ordinal: u64) -> Result<Read, ReadError> {
    let pairs = own_part(pairs);
    let head = common_part(pairs);
    let common = common(head, ordinal)?;
    let space = match int(head, 67)? {
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
            // bulge, a 40 and a 41 its segment's start and end widths. An
            // absent 42 is a straight segment, an absent width none of the
            // vertex's own.
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
                    code @ 40..=42 => {
                        if let Some(last) = vertices.last_mut() {
                            let value = number(&pairs[i])?;
                            match code {
                                40 => last.start_width = value,
                                41 => last.end_width = value,
                                _ => last.bulge = value,
                            }
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let flags = int(pairs, 70)?.unwrap_or(0);
            let const_width = num_or(pairs, 43, 0.0)?;
            Entity::LwPolyline(LwPolylineEntity {
                common,
                vertices: without_restated_widths(vertices, const_width),
                closed: flags & 1 == 1,
                const_width,
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
            let placement = placement(type_name, pairs, 73, &mut warnings)?;
            Entity::Text(TextEntity {
                common,
                start_point: point2(pairs, 10)?,
                text_height: num_or(pairs, 40, 0.0)?,
                text: string(text(pairs, 1).unwrap_or("")),
                rotation: radians(pairs, 50)?,
                horizontal_justification: placement.horizontal,
                vertical_justification: placement.vertical,
                alignment_point: placement.alignment_point,
                width_factor: placement.width_factor,
                oblique_angle: placement.oblique_angle,
                style_name: placement.style_name,
                elevation: num_or(pairs, 30, 0.0)?,
                extrusion: extrusion(pairs)?,
            })
        }
        "ATTRIB" => {
            let placement = placement(type_name, pairs, 74, &mut warnings)?;
            Entity::Attrib(AttribEntity {
                common,
                start_point: point2(pairs, 10)?,
                text_height: num_or(pairs, 40, 0.0)?,
                tag: string(text(pairs, 2).unwrap_or("")),
                flags: attribute_flags(pairs)?,
                text: string(text(pairs, 1).unwrap_or("")),
                rotation: radians(pairs, 50)?,
                horizontal_justification: placement.horizontal,
                vertical_justification: placement.vertical,
                alignment_point: placement.alignment_point,
                width_factor: placement.width_factor,
                oblique_angle: placement.oblique_angle,
                style_name: placement.style_name,
                elevation: num_or(pairs, 30, 0.0)?,
                extrusion: extrusion(pairs)?,
            })
        }
        "ATTDEF" => {
            let placement = placement(type_name, pairs, 74, &mut warnings)?;
            Entity::Attdef(AttdefEntity {
                common,
                start_point: point2(pairs, 10)?,
                text_height: num_or(pairs, 40, 0.0)?,
                tag: string(text(pairs, 2).unwrap_or("")),
                flags: attribute_flags(pairs)?,
                default_value: string(text(pairs, 1).unwrap_or("")),
                rotation: radians(pairs, 50)?,
                horizontal_justification: placement.horizontal,
                vertical_justification: placement.vertical,
                alignment_point: placement.alignment_point,
                width_factor: placement.width_factor,
                oblique_angle: placement.oblique_angle,
                style_name: placement.style_name,
                elevation: num_or(pairs, 30, 0.0)?,
                extrusion: extrusion(pairs)?,
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
                extrusion: extrusion(pairs)?,
            })
        }
        "DIMENSION" | "ARC_DIMENSION" => {
            let flag = int(pairs, 70)?;
            let kind = dimension_kind(type_name, flag, pairs);
            Entity::Dimension(DimensionEntity {
                common,
                block_name: name_ref(text(pairs, 2)),
                kind,
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
                // Bit 64 of group 70 says which coordinate an ordinate dimension
                // measures; on any other kind it means nothing.
                ordinate_axis: (kind == Some(DimensionKind::Ordinate)).then(|| {
                    if flag.unwrap_or(0) & 64 != 0 {
                        OrdinateAxis::X
                    } else {
                        OrdinateAxis::Y
                    }
                }),
            })
        }
        // A feature control frame: its text is the frame's contents, symbol
        // codes and all, as the file wrote it.
        "TOLERANCE" => Entity::Tolerance(ToleranceEntity {
            common,
            insertion_point: point3(pairs, 10)?,
            // The frame's height is its dimension style's; the record
            // states one only in old files.
            text_height: num(pairs, 40)?.filter(|h| *h != 0.0),
            text_value: string(text(pairs, 1).unwrap_or("")),
            // Written only when the frame is turned from the world x axis;
            // absent, it is not turned. A zero vector is no direction.
            direction: Some(optional_point3(pairs, 11)?.unwrap_or(Point3D {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }))
            .filter(|d| d.x != 0.0 || d.y != 0.0 || d.z != 0.0),
            style_name: name_ref(text(pairs, 3)),
        }),
        // Each vertex writes its point (11), its segment's direction (12)
        // and its miter (13), then its elements' parameters; the model
        // carries the point and the miter.
        "MLINE" => Entity::MLine(MLineEntity {
            common,
            vertices: repeated_point3(pairs, 11)?
                .into_iter()
                .zip(repeated_point3(pairs, 13)?)
                .map(|(point, miter_direction)| MLineVertex {
                    point,
                    miter_direction,
                })
                .collect(),
            // 71 bit 2: closed.
            closed: int(pairs, 71)?.is_some_and(|f| f & 2 != 0),
            mlinestyle_name: name_ref(text(pairs, 2)),
            scale: num(pairs, 40)?,
        }),
        "WIPEOUT" => Entity::Wipeout(WipeoutEntity {
            common,
            boundary: wipeout_boundary(pairs)?,
        }),
        // The frame as the file states it, the definition by its handle
        // (resolved once the OBJECTS section is in), and the clip boundary
        // through that frame, as a WIPEOUT's is.
        "IMAGE" => {
            let insertion_point = point3(pairs, 10)?;
            let u_vector = point3(pairs, 11)?;
            let v_vector = point3(pairs, 12)?;
            let size_pixels = Point2D {
                x: num_or(pairs, 13, 0.0)?,
                y: num_or(pairs, 23, 0.0)?,
            };
            Entity::Image(ImageEntity {
                common,
                insertion_point,
                u_vector,
                v_vector,
                size_pixels,
                definition: match text(pairs, 340).map(str::trim) {
                    Some(h) if !h.is_empty() && h != "0" => Ref::Unresolved(h.to_ascii_uppercase()),
                    _ => Ref::Absent,
                },
                display_flags: int(pairs, 70)?.and_then(|v| u16::try_from(v).ok()),
                clipping: int(pairs, 280)?.map(|v| v != 0),
                brightness: int(pairs, 281)?.and_then(|v| u8::try_from(v).ok()),
                contrast: int(pairs, 282)?.and_then(|v| u8::try_from(v).ok()),
                fade: int(pairs, 283)?.and_then(|v| u8::try_from(v).ok()),
                clip_outside: int(pairs, 290)?.map(|v| v != 0),
                boundary: clip_boundary(pairs, insertion_point, u_vector, v_vector, size_pixels)?,
            })
        }
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
            // A measurement the writing application made; a zero measures no
            // text.
            extents_width: num(pairs, 42)?.filter(|w| *w != 0.0),
            extents_height: num(pairs, 43)?.filter(|h| *h != 0.0),
            style_name: name_ref(text(pairs, 7)),
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
                start_tangent: optional_point3(pairs, 12)?,
                end_tangent: optional_point3(pairs, 13)?,
            })
        }
        // The leader lines only, each the points of one `LEADER_LINE{}`
        // block of the record's context data, in file order.
        "MULTILEADER" => Entity::MultiLeader(MultiLeaderEntity {
            common,
            lines: multileader_lines(pairs)?,
        }),
        // A light: where it is, what it aims at (11 -- a point light states
        // it too), and which kind it is. Whether it aims follows from the
        // kind and is left to whoever needs it.
        "LIGHT" => {
            let position = point3(pairs, 10)?;
            Entity::Light(LightEntity {
                common,
                position,
                // The same stand-in the other reader uses when there is none.
                target: optional_point3(pairs, 11)?.unwrap_or(position),
                light_type: match int(pairs, 70)? {
                    Some(1) => Some(LightType::Distant),
                    Some(2) => Some(LightType::Point),
                    Some(3) => Some(LightType::Spot),
                    _ => None,
                },
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
        // A paper-space viewport: its frame on the sheet, and the view of
        // the model it shows through it.
        "VIEWPORT" => Entity::Viewport(ViewportEntity {
            common,
            center: point3(pairs, 10)?,
            width: num_or(pairs, 40, 0.0)?,
            height: num_or(pairs, 41, 0.0)?,
            view: viewport_view(pairs)?,
            on: viewport_on(pairs)?,
            viewport_id: int(pairs, 69)?.map(|id| id as i32),
            // The frozen layers by their LAYER records' handles (341, or 331
            // as a DXF from R2004 on writes them), resolved to names once the
            // tables are in.
            frozen_layers: pairs
                .iter()
                .filter(|p| p.code == 341 || p.code == 331)
                .map(|p| Ref::Unresolved(p.value.trim().to_ascii_uppercase()))
                .collect(),
        }),
        "HATCH" => {
            let hatch = crate::hatch::read(pairs, &mut warnings)?;
            Entity::Hatch(HatchEntity {
                common,
                boundary_paths: hatch.boundary_paths,
                solid_fill: hatch.solid_fill,
                gradient: hatch.gradient,
                pattern_lines: hatch.pattern_lines,
                elevation: hatch.elevation,
                extrusion: hatch.extrusion,
                style: hatch.style,
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
        Some(other) => TextOverride::Literal(string(other)),
    }
}

/// How a TEXT, ATTRIB or ATTDEF is placed beyond its start point: the
/// groups the three share.
struct Placement {
    horizontal: HorizontalJustification,
    vertical: VerticalJustification,
    alignment_point: Option<Point2D>,
    width_factor: f64,
    oblique_angle: f64,
    style_name: Ref<String>,
}

/// Reads a TEXT's, ATTRIB's or ATTDEF's [`Placement`].
///
/// - The justifications are 72 and `vertical` -- 73 for a TEXT, 74 for an
///   attribute, whose 73 is its field length. An absent group is the
///   default; a value outside the format's range is reported, and read as
///   the default rather than refused.
/// - The alignment point (11) is kept only for a justification other than
///   left and baseline -- the only case in which the format writes it.
/// - The width factor (41) is a ratio whose default is 1; a 0 is no width
///   at all, which is what a writer that leaves the group empty means, so
///   it is 1 too.
/// - The oblique angle (51) is written in degrees.
/// - The style (7) is carried by name and resolved against the STYLE table
///   once the tables are in; an absent group is the reference's default,
///   the style named `STANDARD`, which the reader resolves the same way.
fn placement(
    type_name: &str,
    pairs: &[Pair<'_>],
    vertical: i32,
    warnings: &mut Vec<String>,
) -> Result<Placement, ReadError> {
    let horizontal = match int(pairs, 72)? {
        None | Some(0) => HorizontalJustification::Left,
        Some(1) => HorizontalJustification::Center,
        Some(2) => HorizontalJustification::Right,
        Some(3) => HorizontalJustification::Aligned,
        Some(4) => HorizontalJustification::Middle,
        Some(5) => HorizontalJustification::Fit,
        Some(other) => {
            warnings.push(format!(
                "TEXT_ALIGNMENT: a {type_name} states horizontal alignment {other} (group 72), outside 0 to 5; it is read as left"
            ));
            HorizontalJustification::Left
        }
    };
    let vertical_justification = match int(pairs, vertical)? {
        None | Some(0) => VerticalJustification::Baseline,
        Some(1) => VerticalJustification::Bottom,
        Some(2) => VerticalJustification::Middle,
        Some(3) => VerticalJustification::Top,
        Some(other) => {
            warnings.push(format!(
                "TEXT_ALIGNMENT: a {type_name} states vertical alignment {other} (group {vertical}), outside 0 to 3; it is read as baseline"
            ));
            VerticalJustification::Baseline
        }
    };
    let justified = horizontal != HorizontalJustification::Left
        || vertical_justification != VerticalJustification::Baseline;
    Ok(Placement {
        horizontal,
        vertical: vertical_justification,
        alignment_point: if justified {
            optional_point2(pairs, 11)?
        } else {
            None
        },
        width_factor: num(pairs, 41)?.filter(|w| *w != 0.0).unwrap_or(1.0),
        oblique_angle: radians(pairs, 51)?,
        style_name: name_ref(text(pairs, 7)),
    })
}

/// An ATTRIB's or ATTDEF's flags (70), one per bit as the reference names
/// them: 1 invisible, 2 constant, 4 verify, 8 preset. An absent group is no
/// flag set.
fn attribute_flags(pairs: &[Pair<'_>]) -> Result<AttributeFlags, ReadError> {
    let flags = int(pairs, 70)?.unwrap_or(0);
    Ok(AttributeFlags {
        invisible: flags & 1 != 0,
        constant: flags & 2 != 0,
        verify: flags & 4 != 0,
        preset: flags & 8 != 0,
    })
}

/// A polyline's vertices as the model carries them: a file that states the
/// constant width again on every vertex, at both ends, draws the same
/// polyline as one that states it only as the constant width, and the two
/// read the same -- vertices with no width of their own.
pub(crate) fn without_restated_widths(
    mut vertices: Vec<PolylineVertex>,
    const_width: f64,
) -> Vec<PolylineVertex> {
    let restated = const_width != 0.0
        && vertices
            .iter()
            .all(|v| v.start_width == const_width && v.end_width == const_width);
    if restated {
        for v in &mut vertices {
            v.start_width = 0.0;
            v.end_width = 0.0;
        }
    }
    vertices
}

/// Whether a viewport is on. From R2000 on the record states it as bit
/// 0x20000 of its status flags (90), set when it is off -- the same bit the
/// binary format keeps. Group 68 is its place in the stack of active
/// viewports, and 0 there is also what a viewport of a layout that is not
/// the current one is written with, on or not, so it is read only where
/// there are no status flags: 0 off, -1 or a place in the stack on.
fn viewport_on(pairs: &[Pair<'_>]) -> Result<Option<bool>, ReadError> {
    if let Some(flags) = int(pairs, 90)? {
        return Ok(Some(flags & 0x20000 == 0));
    }
    Ok(int(pairs, 68)?.map(|on| on != 0))
}

/// What a viewport shows of the model (12, 45, 17, 16, 51, 42). A record
/// that writes no view centre carries no view -- a viewport older than R2000
/// keeps it in extended data, which is not read. A zero view direction is
/// not a direction and reads as a plan view.
fn viewport_view(pairs: &[Pair<'_>]) -> Result<Option<ViewportView>, ReadError> {
    let Some(center) = optional_point2(pairs, 12)? else {
        return Ok(None);
    };
    let direction = optional_point3(pairs, 16)?
        .filter(|d| d.x != 0.0 || d.y != 0.0 || d.z != 0.0)
        .unwrap_or(Point3D {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        });
    Ok(Some(ViewportView {
        center,
        height: num_or(pairs, 45, 0.0)?,
        target: point3(pairs, 17)?,
        direction,
        twist: radians(pairs, 51)?,
        lens_length: num_or(pairs, 42, 50.0)?,
    }))
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
/// A MULTILEADER's leader lines: the points (10/20/30) inside each
/// `LEADER_LINE{` ... `}` block (groups 304 and 305), in file order. A line
/// with no point is not a line.
fn multileader_lines(pairs: &[Pair<'_>]) -> Result<Vec<Vec<Point3D>>, ReadError> {
    let mut lines = Vec::new();
    let mut current: Option<Vec<Point3D>> = None;
    let mut i = 0;
    while i < pairs.len() {
        let p = &pairs[i];
        match (p.code, p.value.trim()) {
            (304, "LEADER_LINE{") => current = Some(Vec::new()),
            (305, "}") => {
                if let Some(points) = current.take().filter(|l| !l.is_empty()) {
                    lines.push(points);
                }
            }
            (10, _) => {
                if let Some(points) = current.as_mut() {
                    let at = |code: i32| {
                        pairs
                            .get(i + usize::try_from(code / 10 - 1).unwrap_or(0))
                            .filter(|q| q.code == code)
                            .map(number)
                            .transpose()
                    };
                    points.push(Point3D {
                        x: number(p)?,
                        y: at(20)?.unwrap_or(0.0),
                        z: at(30)?.unwrap_or(0.0),
                    });
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(lines)
}

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
    // Undone after the pieces are joined: a piece boundary can fall inside
    // an escape.
    string(&out)
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
/// A WIPEOUT's clip boundary in the entity's local space. The clip
/// vertices (14) are in its image's pixel space, which runs from the
/// image's upper left corner with each pixel's center on a whole number: a
/// vertex `(x, y)` lies at `10 + (x + 0.5) * 11 + (h - 0.5 - y) * 12`, `h`
/// being the image's height in pixels (23). A two-vertex rectangle (71 = 1)
/// is its opposite corners; no vertices at all is the whole image. A first
/// vertex repeated at the end is dropped: the loop is closed without it.
fn wipeout_boundary(pairs: &[Pair<'_>]) -> Result<Vec<Point2D>, ReadError> {
    let origin = point3(pairs, 10)?;
    let u = optional_point3(pairs, 11)?.unwrap_or(Point3D {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    });
    let v = optional_point3(pairs, 12)?.unwrap_or(Point3D {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    });
    let size = Point2D {
        x: num_or(pairs, 13, 1.0)?,
        y: num_or(pairs, 23, 1.0)?,
    };
    clip_boundary(pairs, origin, u, v, size)
}

/// The clip boundary (71, 91, 14) of a raster entity -- an IMAGE, or the
/// WIPEOUT that shares its layout -- put through the entity's frame, as
/// [`wipeout_boundary`] describes.
fn clip_boundary(
    pairs: &[Pair<'_>],
    origin: Point3D,
    u: Point3D,
    v: Point3D,
    size: Point2D,
) -> Result<Vec<Point2D>, ReadError> {
    let vertices: Vec<Point2D> = repeated_point3(pairs, 14)?
        .into_iter()
        .map(|p| Point2D { x: p.x, y: p.y })
        .collect();
    let rect =
        |a: Point2D, b: Point2D| vec![a, Point2D { x: b.x, y: a.y }, b, Point2D { x: a.x, y: b.y }];
    let mut vertices = vertices;
    // A polygon written closed repeats its first vertex at the end; the loop
    // is closed either way, and the repeat is not a vertex.
    if vertices.len() > 2 && vertices.first() == vertices.last() {
        vertices.pop();
    }
    let pixels = if int(pairs, 71)? == Some(1) && vertices.len() == 2 {
        rect(vertices[0], vertices[1])
    } else if !vertices.is_empty() {
        vertices
    } else {
        rect(
            Point2D { x: -0.5, y: -0.5 },
            Point2D {
                x: size.x - 0.5,
                y: size.y - 0.5,
            },
        )
    };
    Ok(pixels
        .into_iter()
        .map(|p| {
            let (a, b) = (p.x + 0.5, size.y - 0.5 - p.y);
            Point2D {
                x: origin.x + a * u.x + b * v.x,
                y: origin.y + a * u.y + b * v.y,
            }
        })
        .collect())
}

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
        Some(n) if !n.is_empty() => Ref::Unresolved(string(n)),
        _ => Ref::Absent,
    }
}
