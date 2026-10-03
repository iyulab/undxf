//! The space blocks: what model space and paper space own is kept in one
//! block each, however the file spells their names.

use undxf::read_str;

#[test]
fn entities_placed_by_space_flag_join_the_space_block_as_the_file_spelled_it() {
    // R13 and R14 files name the space blocks in capitals in their BLOCKS
    // section and place the drawing's entities by flag in ENTITIES.
    let text = "  0\nSECTION\n  2\nBLOCKS\n\
                  0\nBLOCK\n  8\n0\n  2\n*MODEL_SPACE\n 70\n0\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nBLOCK\n  8\n0\n  2\n*PAPER_SPACE\n 70\n0\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nENDSEC\n\
                  0\nSECTION\n  2\nENTITIES\n\
                  0\nLINE\n  8\n0\n 10\n0\n 20\n0\n 11\n1\n 21\n0\n\
                  0\nLINE\n  8\n0\n 67\n1\n 10\n0\n 20\n0\n 11\n2\n 21\n0\n\
                  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    let spaces: Vec<(&str, usize)> = db
        .tables
        .block_records
        .values()
        .map(|b| (b.name.as_str(), b.entities.len()))
        .collect();
    assert_eq!(spaces, [("*MODEL_SPACE", 1), ("*PAPER_SPACE", 1)]);
    assert_eq!(db.entities.len(), 2);
}
