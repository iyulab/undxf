//! A block that states it is an external reference (BLOCK 70 bit 4) carries
//! the referenced drawing's path (1) and whether it is an overlay (bit 8);
//! an ordinary block carries none.

use uncad_model::tables::ExternalReference;
use undxf::read_str;

#[test]
fn an_external_reference_block_carries_its_path() {
    let text = "  0\nSECTION\n  2\nBLOCKS\n\
                  0\nBLOCK\n  8\n0\n  2\nPLAN\n 70\n68\n 10\n0\n 20\n0\n 30\n0\n  3\nPLAN\n  1\n../base/plan.dwg\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nBLOCK\n  8\n0\n  2\nGRID\n 70\n12\n 10\n0\n 20\n0\n 30\n0\n  3\nGRID\n  1\ngrid.dwg\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nBLOCK\n  8\n0\n  2\nB1\n 70\n0\n 10\n0\n 20\n0\n 30\n0\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    let blocks = &db.tables.block_records;
    assert_eq!(
        blocks["PLAN"].external_reference,
        Some(ExternalReference {
            path: "../base/plan.dwg".to_string(),
            overlay: false,
        })
    );
    assert_eq!(
        blocks["GRID"].external_reference,
        Some(ExternalReference {
            path: "grid.dwg".to_string(),
            overlay: true,
        })
    );
    assert_eq!(blocks["B1"].external_reference, None);
}
