//! The HEADER section: every variable the file states, with the groups it
//! wrote for it.

use std::collections::BTreeMap;
use uncad_model::{Point2D, Point3D};

/// The variables a DXF file's HEADER section states, each under its name
/// without the `$` (`"INSUNITS"`), with the groups that follow its group 9
/// in the order the file wrote them. A variable the file does not state is
/// not here -- which is how a reader tells "stated" from "the format's
/// default", a difference the file's own values cannot show.
///
/// Values are kept as written; the accessors read the first group as a
/// number, a string or a point. What a variable means, and which ones
/// matter, is the caller's: this is the section as the file has it.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Header {
    pub variables: BTreeMap<String, Vec<HeaderGroup>>,
}

/// One group of a header variable: its group code and its value as the
/// file wrote it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HeaderGroup {
    pub code: i32,
    pub value: String,
}

impl HeaderGroup {
    pub(crate) fn new(code: i32, value: &str) -> HeaderGroup {
        HeaderGroup {
            code,
            value: value.to_string(),
        }
    }
}

impl Header {
    /// The groups the file wrote for `name` (without the `$`); `None` when
    /// it does not state the variable.
    pub fn groups(&self, name: &str) -> Option<&[HeaderGroup]> {
        self.variables.get(name).map(Vec::as_slice)
    }

    /// `name`'s first group as an integer (a flag, a code, a count).
    pub fn int(&self, name: &str) -> Option<i64> {
        self.first(name)?.value.trim().parse().ok()
    }

    /// `name`'s first group as a real number.
    pub fn real(&self, name: &str) -> Option<f64> {
        self.first(name)?.value.trim().parse().ok()
    }

    /// `name`'s first group as a string, its storage in the file undone
    /// (`\U+` and `\M+` escapes, caret notation) the way every string this
    /// crate reads is.
    pub fn text(&self, name: &str) -> Option<String> {
        Some(crate::decode::string(&self.first(name)?.value))
    }

    /// `name` as a point: its groups 10, 20 and 30. `None` unless all three
    /// are there and read as numbers.
    pub fn point3(&self, name: &str) -> Option<Point3D> {
        Some(Point3D {
            x: self.coordinate(name, 10)?,
            y: self.coordinate(name, 20)?,
            z: self.coordinate(name, 30)?,
        })
    }

    /// `name` as a planar point: its groups 10 and 20.
    pub fn point2(&self, name: &str) -> Option<Point2D> {
        Some(Point2D {
            x: self.coordinate(name, 10)?,
            y: self.coordinate(name, 20)?,
        })
    }

    fn first(&self, name: &str) -> Option<&HeaderGroup> {
        self.variables.get(name)?.first()
    }

    fn coordinate(&self, name: &str, code: i32) -> Option<f64> {
        self.variables
            .get(name)?
            .iter()
            .find(|g| g.code == code)?
            .value
            .trim()
            .parse()
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(entries: &[(&str, &[(i32, &str)])]) -> Header {
        Header {
            variables: entries
                .iter()
                .map(|(name, groups)| {
                    (
                        name.to_string(),
                        groups
                            .iter()
                            .map(|&(c, v)| HeaderGroup::new(c, v))
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_accessors_read_the_first_group_by_kind() {
        let h = header(&[
            ("INSUNITS", &[(70, "     4")]),
            ("DIMSCALE", &[(40, "2.5")]),
            ("DIMPOST", &[(1, "<> mm")]),
            ("EXTMIN", &[(10, "-1"), (20, "2"), (30, "0.5")]),
            ("LIMMAX", &[(10, "420"), (20, "297")]),
        ]);
        assert_eq!(h.int("INSUNITS"), Some(4));
        assert_eq!(h.real("DIMSCALE"), Some(2.5));
        assert_eq!(h.text("DIMPOST").as_deref(), Some("<> mm"));
        assert_eq!(
            h.point3("EXTMIN"),
            Some(Point3D {
                x: -1.0,
                y: 2.0,
                z: 0.5
            })
        );
        assert_eq!(h.point2("LIMMAX"), Some(Point2D { x: 420.0, y: 297.0 }));
        // A planar variable is no 3D point; an unstated one is nothing.
        assert_eq!(h.point3("LIMMAX"), None);
        assert_eq!(h.int("LUNITS"), None);
    }

    #[test]
    fn a_string_value_is_undone_like_every_other_string() {
        let h = header(&[("DIMPOST", &[(1, "<> \\U+00B0")])]);
        assert_eq!(h.text("DIMPOST").as_deref(), Some("<> \u{b0}"));
    }
}
