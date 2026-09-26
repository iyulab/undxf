//! A dimension or leader that sets style variables for itself: the `DSTYLE`
//! list in its extended data under `ACAD`, read as the file states it.

use std::path::{Path, PathBuf};
use uncad_model::model::{Entity, OverrideValue, StyleOverride};
use undxf::read_str;

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

fn drawing(entities: &str) -> String {
    format!(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n{entities}  0\nENDSEC\n  0\nEOF\n"
    )
}

const DIMENSION: &str = "  0\nDIMENSION\n  5\n2A\n  8\n0\n  2\n*D1\n 10\n0\n 20\n0\n 30\n0\n 11\n5\n 21\n1\n 31\n0\n 70\n0\n  3\nSTANDARD\n 13\n0\n 23\n0\n 33\n0\n 14\n10\n 24\n0\n 34\n0\n";

fn overrides(entity: &Entity) -> Option<&Vec<StyleOverride>> {
    match entity {
        Entity::Dimension(d) => d.style_overrides.as_ref(),
        Entity::Leader(l) => l.style_overrides.as_ref(),
        _ => None,
    }
}

#[test]
fn every_kind_of_value_is_read_in_file_order() {
    let eed = concat!(
        "1001\nACAD\n1000\nDSTYLE\n1002\n{\n",
        "1070\n41\n1040\n0.24\n",
        "1070\n271\n1070\n0\n",
        "1070\n3\n1000\n<> mm\n",
        "1070\n341\n1005\n77A\n",
        "1002\n}\n"
    );
    let db = read_str(&drawing(&format!("{DIMENSION}{eed}"))).unwrap();
    let got = overrides(&db.entities[0]).expect("the reader looked");
    let expected = [
        (41, OverrideValue::Real(0.24)),
        (271, OverrideValue::Integer(0)),
        (3, OverrideValue::Text("<> mm".to_string())),
        (341, OverrideValue::Handle("77A".to_string())),
    ];
    let got: Vec<(u16, OverrideValue)> =
        got.iter().map(|o| (o.variable, o.value.clone())).collect();
    assert_eq!(got, expected);
}

#[test]
fn no_list_is_an_empty_list_and_other_applications_are_not_read() {
    let other = "1001\nOTHERAPP\n1000\nDSTYLE\n1002\n{\n1070\n41\n1040\n9\n1002\n}\n";
    let db = read_str(&drawing(&format!("{DIMENSION}{DIMENSION}{other}"))).unwrap();
    for entity in &db.entities {
        assert_eq!(overrides(entity), Some(&Vec::new()), "{entity:?}");
    }
}

fn dxfs_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            dxfs_under(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dxf"))
        {
            out.push(path);
        }
    }
}

/// The corpus carries `DSTYLE` lists on 7 dimensions and 9 leaders (an R12
/// file also has lists on records that are neither, which this reader does
/// not look at). Each is read whole, in order.
#[test]
fn the_corpus_lists_are_read_whole() {
    let mut files = Vec::new();
    dxfs_under(Path::new(CORPUS), &mut files);
    files.sort();
    let (mut dimensions, mut leaders) = (0, 0);
    let mut seen = Vec::new();
    for file in &files {
        let Ok(db) = undxf::read_bytes(&std::fs::read(file).unwrap()) else {
            continue;
        };
        let mut ids = std::collections::BTreeSet::new();
        for entity in db.all_entities() {
            // Model space is listed at the top level and in its block record.
            if !ids.insert(entity.common().id) {
                continue;
            }
            let Some(list) = overrides(entity).filter(|l| !l.is_empty()) else {
                continue;
            };
            match entity {
                Entity::Dimension(_) => dimensions += 1,
                _ => leaders += 1,
            }
            let name = file
                .strip_prefix(CORPUS)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let variables: Vec<u16> = list.iter().map(|o| o.variable).collect();
            seen.push((name, entity.type_name().to_string(), variables));
        }
    }
    assert_eq!((dimensions, leaders), (7, 9), "{seen:#?}");
    assert!(
        seen.contains(&(
            "example_r12.dxf".to_string(),
            "DIMENSION".to_string(),
            vec![271]
        )),
        "{seen:#?}"
    );
    assert!(
        seen.contains(&(
            "2018/Leader.dxf".to_string(),
            "LEADER".to_string(),
            vec![40, 41, 341, 147, 77]
        )),
        "{seen:#?}"
    );
}
