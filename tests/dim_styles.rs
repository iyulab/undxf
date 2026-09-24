//! The DIMSTYLE table: a style variable is written only when it differs from
//! the value the application starts from.

use uncad_model::tables::{AngularUnitFormat, FractionFormat, LinearUnitFormat};
use undxf::read_str;

/// A table with two styles: one that states nothing but its name, and one
/// that states a value for every variable read here.
const DRAWING: &str = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  0\nENDSEC\n  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nDIMSTYLE\n 70\n2\n  0\nDIMSTYLE\n100\nAcDbSymbolTableRecord\n100\nAcDbDimStyleTableRecord\n  2\nPLAIN\n 70\n0\n  0\nDIMSTYLE\n100\nAcDbSymbolTableRecord\n100\nAcDbDimStyleTableRecord\n  2\nFULL\n 70\n0\n  3\n<>mm\n 40\n2\n 41\n2.5\n 45\n0.5\n 47\n0.1\n 48\n0.2\n 71\n1\n 72\n1\n 78\n8\n140\n2.5\n144\n25.4\n179\n1\n271\n2\n272\n3\n275\n1\n276\n2\n277\n4\n  0\nENDTAB\n  0\nENDSEC\n  0\nEOF\n";

#[test]
fn an_unwritten_variable_is_its_starting_value_where_every_template_shares_it() {
    let db = read_str(DRAWING).unwrap();
    let s = &db.tables.dim_styles["PLAIN"];
    assert_eq!(s.post.as_deref(), Some(""));
    assert_eq!(s.scale, Some(1.0));
    assert_eq!(s.length_factor, Some(1.0));
    assert_eq!(s.tolerances, Some(false));
    assert_eq!(s.limits, Some(false));
    assert_eq!(s.tolerance_upper, Some(0.0));
    assert_eq!(s.tolerance_lower, Some(0.0));
    assert_eq!(s.rounding, Some(0.0));
    assert_eq!(s.linear_unit_format, Some(LinearUnitFormat::Decimal));
    assert_eq!(
        s.angular_unit_format,
        Some(AngularUnitFormat::DecimalDegrees)
    );
    assert_eq!(s.angular_decimal_places, Some(0));
    assert_eq!(s.fraction_format, Some(FractionFormat::Horizontal));
    // The templates start these differently (imperial 0.18 / 4 / 0, metric
    // 2.5 / 2 / 8): unwritten, the style does not state them.
    assert_eq!(s.text_height, None);
    assert_eq!(s.arrow_size, None);
    assert_eq!(s.decimal_places, None);
    assert_eq!(s.tolerance_decimal_places, None);
    assert_eq!(s.zero_suppression, None);
}

#[test]
fn a_written_variable_is_read_as_the_file_states_it() {
    let db = read_str(DRAWING).unwrap();
    let s = &db.tables.dim_styles["FULL"];
    assert_eq!(s.post.as_deref(), Some("<>mm"));
    assert_eq!(s.scale, Some(2.0));
    assert_eq!(s.length_factor, Some(25.4));
    assert_eq!(s.tolerances, Some(true));
    assert_eq!(s.limits, Some(true));
    assert_eq!(s.tolerance_upper, Some(0.1));
    assert_eq!(s.tolerance_lower, Some(0.2));
    assert_eq!(s.rounding, Some(0.5));
    assert_eq!(s.linear_unit_format, Some(LinearUnitFormat::Architectural));
    assert_eq!(
        s.angular_unit_format,
        Some(AngularUnitFormat::DegreesMinutesSeconds)
    );
    assert_eq!(s.angular_decimal_places, Some(1));
    assert_eq!(s.fraction_format, Some(FractionFormat::NotStacked));
    assert_eq!(s.text_height, Some(2.5));
    assert_eq!(s.arrow_size, Some(2.5));
    assert_eq!(s.decimal_places, Some(2));
    assert_eq!(s.tolerance_decimal_places, Some(3));
    assert_eq!(s.zero_suppression, Some(8));
}

#[test]
fn a_variable_that_came_with_r2000_is_not_stated_by_an_earlier_file() {
    let r14 = DRAWING.replace("AC1015", "AC1014");
    let db = read_str(&r14).unwrap();
    let s = &db.tables.dim_styles["PLAIN"];
    assert_eq!(s.linear_unit_format, None);
    assert_eq!(s.fraction_format, None);
    assert_eq!(s.angular_decimal_places, None);
    // The others were there before: unwritten, still their starting value.
    assert_eq!(s.tolerance_upper, Some(0.0));
    assert_eq!(
        s.angular_unit_format,
        Some(AngularUnitFormat::DecimalDegrees)
    );
    // No header at all: the version is unknown, so the same.
    let bare = DRAWING.replace(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  0\nENDSEC\n",
        "",
    );
    let db = read_str(&bare).unwrap();
    assert_eq!(db.tables.dim_styles["PLAIN"].linear_unit_format, None);
}

#[test]
fn before_r2000_one_variable_says_both_formats() {
    // DIMUNIT (270): 6 is architectural without stacked fractions, 4 the
    // same with them -- how they are stacked it does not say.
    let with = |value: &str| {
        DRAWING.replace("AC1015", "AC1014").replace(
            "  2\nPLAIN\n 70\n0\n",
            &format!("  2\nPLAIN\n 70\n0\n270\n{value}\n"),
        )
    };
    let s = |text: String| read_str(&text).unwrap().tables.dim_styles["PLAIN"].clone();
    let six = s(with("6"));
    assert_eq!(
        six.linear_unit_format,
        Some(LinearUnitFormat::Architectural)
    );
    assert_eq!(six.fraction_format, Some(FractionFormat::NotStacked));
    let four = s(with("4"));
    assert_eq!(
        four.linear_unit_format,
        Some(LinearUnitFormat::Architectural)
    );
    assert_eq!(four.fraction_format, None);
    let two = s(with("2"));
    assert_eq!(two.linear_unit_format, Some(LinearUnitFormat::Decimal));
    assert_eq!(two.fraction_format, None);
    // From R2000 on the variable is not read: DIMLUNIT and DIMFRAC say it.
    let modern = read_str(&DRAWING.replace("  2\nPLAIN\n 70\n0\n", "  2\nPLAIN\n 70\n0\n270\n6\n"))
        .unwrap()
        .tables
        .dim_styles["PLAIN"]
        .clone();
    assert_eq!(modern.linear_unit_format, Some(LinearUnitFormat::Decimal));
}
