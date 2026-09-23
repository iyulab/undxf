//! One walk over every DXF of the LibreDWG test corpus, when it is present
//! beside this crate (it is a GPL project's data and is not copied here):
//! how each file read, how many entities came back, and which entity types
//! this crate kept as `UNKNOWN`. The counts are printed so a change shows up
//! as a number, and pinned so that a change fails the build rather than
//! passing in silence.
//!
//! The pinned values are measurements, not requirements: when the corpus or
//! the reader legitimately changes, re-measure and update them -- but say why.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uncad_model::model::Entity;

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../uncad/lib/libredwg/test/test-data"
);

fn dxfs_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("corpus directory should be readable") {
        let path = entry.expect("corpus entry should be readable").path();
        if path.is_dir() {
            dxfs_under(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dxf"))
        {
            out.push(path);
        }
    }
}

#[derive(Default)]
struct Sweep {
    files: usize,
    read: usize,
    /// Error reason -> number of files.
    errors: BTreeMap<String, usize>,
    /// Files whose bytes were not valid UTF-8, decoded through the code
    /// page their header declares.
    code_page: usize,
    /// Files whose read raised a diagnostic, with the diagnostics.
    diagnostics: Vec<(String, Vec<String>)>,
    entities: usize,
    /// Entity type -> count, over top-level and block entities, for the
    /// types this crate does not interpret.
    unknown: BTreeMap<String, usize>,
    /// Entity type -> count, for the types it does.
    known: BTreeMap<String, usize>,
    /// (relative path, top-level entity count) per file that read.
    per_file: Vec<(String, usize)>,
}

#[test]
fn corpus_sweep() {
    let root = Path::new(CORPUS);
    if !root.is_dir() {
        println!(
            "skipped -- no corpus at {CORPUS} (a tree carrying uncad beside this crate has it)"
        );
        return;
    }
    let mut files = Vec::new();
    dxfs_under(root, &mut files);
    files.sort();
    let mut s = Sweep::default();
    for path in &files {
        s.files += 1;
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(path).expect("corpus file should be readable");
        if std::str::from_utf8(&bytes).is_err() {
            s.code_page += 1;
        }
        match undxf::read_bytes(&bytes) {
            Err(e) => {
                let reason = serde_json::to_value(&e).unwrap()["reason"]
                    .as_str()
                    .unwrap()
                    .to_string();
                *s.errors.entry(reason).or_default() += 1;
                println!("{rel}: ERROR {e}");
            }
            Ok(db) => {
                s.read += 1;
                if !db.read_diagnostics.is_clean() {
                    s.diagnostics
                        .push((rel.clone(), db.read_diagnostics.warnings.clone()));
                }
                s.entities += db.entities.len();
                s.per_file.push((rel.clone(), db.entities.len()));
                for e in db.all_entities() {
                    match e {
                        Entity::Unknown { type_name, .. } => {
                            *s.unknown.entry(type_name.clone()).or_default() += 1
                        }
                        other => *s.known.entry(other.type_name().to_string()).or_default() += 1,
                    }
                }
            }
        }
    }
    println!(
        "files {} read {} code-page {} with-diagnostics {}",
        s.files,
        s.read,
        s.code_page,
        s.diagnostics.len()
    );
    for (f, w) in &s.diagnostics {
        println!("  {f}: {w:?}");
    }
    println!("errors {:?}", s.errors);
    println!("top-level entities {}", s.entities);
    println!("known {:?}", s.known);
    println!("unknown {:?}", s.unknown);
    for (f, n) in &s.per_file {
        println!("  {f}: {n}");
    }

    // Pinned 2026-09-22 on libredwg a8ce248's test-data. The one file that
    // does not read is a pre-R10 drawing (r1.4/entities.dxf) whose first
    // line is not a group code: that format is not the pair form this crate
    // reads, and the error says so at line 1.
    assert_eq!(s.files, 67);
    assert_eq!(s.read, 66, "{:?}", s.errors);
    assert_eq!(s.errors.get("BAD_GROUP_CODE"), Some(&1));
    assert_eq!(
        s.code_page, 4,
        "the four pre-2007 files with code-page text"
    );
    assert!(
        s.diagnostics.is_empty(),
        "every code page in the corpus is known and every byte decodes: {:?}",
        s.diagnostics
    );
    assert_eq!(s.entities, 1225);
    assert_eq!(s.known.get("LINE"), Some(&49345));
    assert_eq!(s.known.get("POLYLINE_2D"), Some(&66));
    assert_eq!(s.known.get("POLYLINE_3D"), Some(&27));
    assert_eq!(s.known.get("INSERT"), Some(&566));
    // Leaders read since this crate learned the entity; the multi-leader is
    // a different entity and is not one of them.
    assert_eq!(s.known.get("LEADER"), Some(&20));
    assert_eq!(s.unknown.get("LEADER"), None);
    assert_eq!(s.unknown.get("MULTILEADER"), Some(&24));
    assert_eq!(s.known.get("MTEXT"), Some(&308));
    assert_eq!(s.unknown.get("MTEXT"), None);
    assert_eq!(s.unknown.get("VIEWPORT"), Some(&64));
    // Points and the planar and 3D faces, read since this crate learned them.
    assert_eq!(s.known.get("POINT"), Some(&789));
    assert_eq!(s.known.get("SOLID"), Some(&266));
    assert_eq!(s.known.get("TRACE"), Some(&20));
    assert_eq!(s.known.get("3DFACE"), Some(&182));
    for t in ["POINT", "SOLID", "TRACE", "3DFACE"] {
        assert_eq!(s.unknown.get(t), None, "{t}");
    }
    assert_eq!(
        s.unknown.get("VERTEX"),
        None,
        "vertices fold into their polylines"
    );
    assert_eq!(s.unknown.get("SEQEND"), None, "sequence ends are structure");
    // The same drawing saved as 2000 through 2018 reads to the same count
    // from every version -- the other reader of this model returns nothing
    // from the 2007+ files of this set.
    let count = |name: &str| s.per_file.iter().find(|(f, _)| f == name).map(|(_, n)| *n);
    for version in ["2000", "2004", "2007", "2010", "2013", "2018"] {
        assert_eq!(
            count(&format!("example_{version}.dxf")),
            Some(72),
            "example_{version}"
        );
        assert_eq!(
            count(&format!("sample_{version}.dxf")),
            Some(6),
            "sample_{version}"
        );
    }
    assert_eq!(count("2018/Dynblocks.dxf"), Some(121));
    assert_eq!(count("2000/TS1.dxf"), Some(34));
}
