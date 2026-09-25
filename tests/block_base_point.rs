//! A block's base point (BLOCK 10/20/30): the point of the definition a
//! block reference puts on its insertion point. An absent group is the
//! format's default, the origin.

use undxf::read_str;

#[test]
fn a_block_carries_its_base_point() {
    let text = "  0\nSECTION\n  2\nBLOCKS\n\
                  0\nBLOCK\n  8\n0\n  2\nB1\n 70\n0\n 10\n6\n 20\n1\n 30\n0\n\
                  0\nLINE\n  8\n0\n 10\n6\n 20\n1\n 11\n7\n 21\n2\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nBLOCK\n  8\n0\n  2\nB2\n 70\n0\n\
                  0\nENDBLK\n  8\n0\n\
                  0\nENDSEC\n  0\nEOF\n";
    let db = read_str(text).unwrap();
    let b1 = &db.tables.block_records["B1"];
    assert_eq!((b1.base_point.x, b1.base_point.y), (6.0, 1.0));
    // The entities keep the coordinates the file gives them.
    assert_eq!(b1.entities.len(), 1);
    let b2 = &db.tables.block_records["B2"];
    assert_eq!(
        (b2.base_point.x, b2.base_point.y, b2.base_point.z),
        (0.0, 0.0, 0.0)
    );
}
