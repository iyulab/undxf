//! HATCH: its boundary paths, its fill and its pattern.
//!
//! Unlike the other entities, a HATCH record cannot be read by looking a
//! group code up: it repeats codes, and only their position tells them
//! apart -- 10, 20 and 30 are the elevation point, then every boundary vertex and
//! edge point, then the seed points; 72 and 73 mean one thing in a polyline
//! path and another in an edge. So it is read in order, with the counts the
//! record states (91 paths, 93 vertices or edges, 78 pattern lines, 453
//! gradient colors) saying how much follows.
//!
//! A record that runs out before a count is met, or that has something else
//! where the order says a group goes, keeps what was read and is reported as
//! `HATCH_STRUCTURE`.

use crate::pairs::{Pair, ReadError};
use uncad_model::model::{
    HatchBoundaryPath, HatchEdge, HatchGradient, HatchPatternLine, Point2D, Point3D, PolylineVertex,
};

/// What a HATCH states besides the fields every entity carries.
pub(crate) struct Hatch {
    pub boundary_paths: Vec<HatchBoundaryPath>,
    pub solid_fill: bool,
    pub gradient: Option<HatchGradient>,
    pub pattern_lines: Vec<HatchPatternLine>,
    pub elevation: f64,
    pub extrusion: Point3D,
}

/// Reads the HATCH-specific part of `pairs`, in order.
pub(crate) fn read(pairs: &[Pair<'_>], warnings: &mut Vec<String>) -> Result<Hatch, ReadError> {
    let start = pairs
        .iter()
        .position(|p| p.code == 100 && p.value.trim() == "AcDbHatch")
        .map_or(0, |i| i + 1);
    let mut c = Cursor {
        pairs: &pairs[start..],
        at: 0,
    };
    let mut hatch = Hatch {
        boundary_paths: Vec::new(),
        solid_fill: false,
        gradient: None,
        pattern_lines: Vec::new(),
        elevation: 0.0,
        extrusion: Point3D {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    };
    // The header: the elevation point (only its z says anything), the
    // extrusion, the fill flag, then the path count.
    let mut extrusion: [Option<f64>; 3] = [None; 3];
    let mut path_count = None;
    while let Some(p) = c.next() {
        match p.code {
            30 => hatch.elevation = number(p)?,
            210 => extrusion[0] = Some(number(p)?),
            220 => extrusion[1] = Some(number(p)?),
            230 => extrusion[2] = Some(number(p)?),
            70 => hatch.solid_fill = integer(p)? != 0,
            91 => {
                path_count = Some(count(p)?);
                break;
            }
            _ => {}
        }
    }
    if extrusion[0].is_some() {
        // Written only when it is not the default; a component the record
        // leaves out is 0, as for any point.
        hatch.extrusion = Point3D {
            x: extrusion[0].unwrap_or(0.0),
            y: extrusion[1].unwrap_or(0.0),
            z: extrusion[2].unwrap_or(0.0),
        };
    }
    let Some(path_count) = path_count else {
        return Ok(hatch);
    };
    for _ in 0..path_count {
        match path(&mut c)? {
            Ok(path) => hatch.boundary_paths.push(path),
            Err(what) => {
                warnings.push(structure(what));
                return Ok(hatch);
            }
        }
    }
    // The pattern: its line families, each with its dashes.
    let rest = &c.pairs[c.at..];
    if let Some(i) = rest.iter().position(|p| p.code == 78) {
        let mut c = Cursor {
            pairs: &rest[i..],
            at: 0,
        };
        let n = count(c.next().expect("found above"))?;
        for _ in 0..n {
            match pattern_line(&mut c)? {
                Ok(line) => hatch.pattern_lines.push(line),
                Err(what) => {
                    warnings.push(structure(what));
                    break;
                }
            }
        }
    }
    hatch.gradient = gradient(rest)?;
    Ok(hatch)
}

fn structure(what: String) -> String {
    format!("HATCH_STRUCTURE: {what}; what was read before it is kept")
}

/// One boundary path. The outer `Result` is a value that is not a number;
/// the inner `Err` says where the record stops making sense.
fn path(c: &mut Cursor<'_, '_>) -> Result<Result<HatchBoundaryPath, String>, ReadError> {
    let Some(flags) = c.take(92) else {
        return Ok(Err(
            "a boundary path does not start with its type (92)".into()
        ));
    };
    let flags = integer(flags)?;
    let path = if flags & 2 != 0 {
        // A polyline path: whether its vertices carry bulges, whether it is
        // closed (a boundary always is), and its vertices.
        let has_bulge = match c.take(72) {
            Some(p) => integer(p)? != 0,
            None => false,
        };
        c.take(73);
        let Some(n) = c.take(93) else {
            return Ok(Err("a polyline path states no vertex count (93)".into()));
        };
        let n = count(n)?;
        let mut vertices = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            let Some(point) = c.point(10)? else {
                return Ok(Err(format!(
                    "a polyline path states {n} vertices but has {}",
                    vertices.len()
                )));
            };
            let bulge = if has_bulge {
                c.take(42).map(number).transpose()?.unwrap_or(0.0)
            } else {
                0.0
            };
            vertices.push(PolylineVertex { point, bulge });
        }
        HatchBoundaryPath::Polyline(vertices)
    } else {
        let Some(n) = c.take(93) else {
            return Ok(Err("an edge path states no edge count (93)".into()));
        };
        let n = count(n)?;
        let mut edges = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            match edge(c)? {
                Ok(Some(e)) => edges.push(e),
                Ok(None) => {}
                Err(what) => return Ok(Err(what)),
            }
        }
        HatchBoundaryPath::Edges(edges)
    };
    // The objects the boundary was picked from: references, not geometry.
    if let Some(n) = c.take(97) {
        for _ in 0..count(n)? {
            c.take(330);
        }
    }
    Ok(Ok(path))
}

/// One edge of an edge path; `None` for a kind the model has no edge for.
fn edge(c: &mut Cursor<'_, '_>) -> Result<Result<Option<HatchEdge>, String>, ReadError> {
    let Some(kind) = c.take(72) else {
        return Ok(Err("an edge does not start with its kind (72)".into()));
    };
    let missing = |what: &str| Ok(Err(format!("an edge carries no {what}")));
    Ok(Ok(Some(match integer(kind)? {
        1 => {
            let Some(start) = c.point(10)? else {
                return missing("start point");
            };
            c.point(11)?;
            HatchEdge::Line { start }
        }
        2 => {
            let Some(center) = c.point(10)? else {
                return missing("center");
            };
            HatchEdge::Arc {
                center,
                radius: c.number(40)?.unwrap_or(0.0),
                start_angle: c.number(50)?.unwrap_or(0.0).to_radians(),
                end_angle: c.number(51)?.unwrap_or(0.0).to_radians(),
                is_ccw: c.number(73)?.is_some_and(|v| v != 0.0),
            }
        }
        3 => {
            let Some(center) = c.point(10)? else {
                return missing("center");
            };
            let Some(end) = c.point(11)? else {
                return missing("major axis endpoint");
            };
            HatchEdge::Ellipse {
                center,
                end,
                minor_major_ratio: c.number(40)?.unwrap_or(0.0),
                start_angle: c.number(50)?.unwrap_or(0.0).to_radians(),
                end_angle: c.number(51)?.unwrap_or(0.0).to_radians(),
                is_ccw: c.number(73)?.is_some_and(|v| v != 0.0),
            }
        }
        4 => {
            c.take(94);
            let rational = c.number(73)?.is_some_and(|v| v != 0.0);
            c.take(74);
            let knots = c.take(95).map(count).transpose()?.unwrap_or(0);
            let Some(n) = c.take(96) else {
                return missing("control point count (96)");
            };
            let n = count(n)?;
            for _ in 0..knots {
                c.take(40);
            }
            let mut control_points = Vec::with_capacity(n.min(1 << 16));
            for _ in 0..n {
                let Some(point) = c.point(10)? else {
                    return Ok(Err(format!(
                        "a spline edge states {n} control points but has {}",
                        control_points.len()
                    )));
                };
                control_points.push(point);
            }
            if rational {
                for _ in 0..n {
                    c.take(42);
                }
            }
            if let Some(fit) = c.take(97) {
                for _ in 0..count(fit)? {
                    c.point(11)?;
                }
            }
            c.point(12)?;
            c.point(13)?;
            HatchEdge::Spline { control_points }
        }
        _ => return Ok(Ok(None)),
    })))
}

/// One pattern line family (53 angle, 43/44 base point, 45/46 offset, 79
/// dash count, 49 dashes).
fn pattern_line(c: &mut Cursor<'_, '_>) -> Result<Result<HatchPatternLine, String>, ReadError> {
    let Some(angle) = c.take(53) else {
        return Ok(Err(
            "a pattern line does not start with its angle (53)".into()
        ));
    };
    let angle = number(angle)?.to_radians();
    let base_point = c.pair(43, 44)?.unwrap_or_default();
    let offset = c.pair(45, 46)?.unwrap_or_default();
    let dashes = c.take(79).map(count).transpose()?.unwrap_or(0);
    let mut dash_pattern = Vec::with_capacity(dashes.min(1 << 16));
    for _ in 0..dashes {
        match c.take(49) {
            Some(d) => dash_pattern.push(number(d)?),
            None => {
                return Ok(Err(format!(
                    "a pattern line states {dashes} dashes but has {}",
                    dash_pattern.len()
                )))
            }
        }
    }
    Ok(Ok(HatchPatternLine {
        angle,
        base_point,
        offset,
        dash_pattern,
    }))
}

/// The gradient, when 450 says the fill is one: its colors (453 records of
/// 463 position and 421 true color or 63 color index), 452 single color,
/// 460 angle (radians), 462 tint, 470 name. `None` for no gradient, or one
/// with no color the file states.
fn gradient(pairs: &[Pair<'_>]) -> Result<Option<HatchGradient>, ReadError> {
    let find = |code: i32| pairs.iter().find(|p| p.code == code);
    if find(450).map(integer).transpose()? != Some(1) {
        return Ok(None);
    }
    // Each color record is a 463 followed by its color.
    let mut stops: Vec<(f64, u32)> = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].code == 463 {
            let position = number(&pairs[i])?;
            let mut color = None;
            let mut j = i + 1;
            while j < pairs.len() && pairs[j].code != 463 && pairs[j].code != 460 {
                match pairs[j].code {
                    421 => color = Some((integer(&pairs[j])? as u32) & 0xff_ffff),
                    63 if color.is_none() => {
                        color = uncad_model::color::aci_to_rgb(
                            integer(&pairs[j])?.unsigned_abs() as u16
                        )
                    }
                    _ => {}
                }
                j += 1;
            }
            stops.push((position, color.unwrap_or(0)));
            i = j;
        } else {
            i += 1;
        }
    }
    let single = find(452).map(integer).transpose()? == Some(1);
    let tint = find(462).map(number).transpose()?.unwrap_or(0.0);
    let (color1, color2, tint) = if single {
        let Some(&(_, c)) = stops.first() else {
            return Ok(None);
        };
        (c, None, tint)
    } else if stops.len() >= 2 {
        stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        (stops[0].1, Some(stops[stops.len() - 1].1), 0.0)
    } else {
        let Some(&(_, c)) = stops.first() else {
            return Ok(None);
        };
        (c, Some(c), 0.0)
    };
    let name = find(470).map(|p| p.value.trim().to_ascii_uppercase());
    Ok(Some(HatchGradient {
        is_radial: matches!(name.as_deref(), Some("SPHERICAL" | "HEMISPHERICAL")),
        angle: find(460).map(number).transpose()?.unwrap_or(0.0),
        color1,
        color2,
        tint,
    }))
}

/// A walk through a record's pairs in order.
struct Cursor<'p, 'a> {
    pairs: &'p [Pair<'a>],
    at: usize,
}

impl<'p, 'a> Cursor<'p, 'a> {
    fn next(&mut self) -> Option<&'p Pair<'a>> {
        let p = self.pairs.get(self.at)?;
        self.at += 1;
        Some(p)
    }

    /// The next pair, if it has `code`.
    fn take(&mut self, code: i32) -> Option<&'p Pair<'a>> {
        let p = self.pairs.get(self.at).filter(|p| p.code == code)?;
        self.at += 1;
        Some(p)
    }

    fn number(&mut self, code: i32) -> Result<Option<f64>, ReadError> {
        self.take(code).map(number).transpose()
    }

    /// The point `x`/`x + 10` next, if it is there.
    fn point(&mut self, x: i32) -> Result<Option<Point2D>, ReadError> {
        self.pair(x, x + 10)
    }

    /// The point whose coordinates are groups `x` and `y`, next, if it is
    /// there. A pattern line's base point and offset pair 43 with 44 and 45
    /// with 46, not with the group ten above.
    fn pair(&mut self, x: i32, y: i32) -> Result<Option<Point2D>, ReadError> {
        let Some(px) = self.take(x) else {
            return Ok(None);
        };
        let x_value = number(px)?;
        let y = self.number(y)?.unwrap_or(0.0);
        Ok(Some(Point2D { x: x_value, y }))
    }
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

/// A count the record states; a negative one is none.
fn count(p: &Pair<'_>) -> Result<usize, ReadError> {
    Ok(usize::try_from(integer(p)?).unwrap_or(0))
}
