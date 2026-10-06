# Changelog

Notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versioning follows
[Semantic Versioning](https://semver.org/). While the version is 0.x, a breaking change
bumps the minor version.

## [Unreleased]

### Added

- A table's `flow`: its own flow direction (group 70 among its table-level groups) when it states
  one, otherwise the flow of the TABLESTYLE it points at (342), read from the OBJECTS section. A
  code the format does not define is not read and is reported (`TABLE_FLOW`); a style that is not
  in the drawing, or states none, leaves it `None`.

## [0.4.0] - 2026-10-05

### Added

- An ACAD_TABLE's grid: its column widths (142), and its rows (141 heights) with one cell per
  column -- the cell's kind (171), whether another cell's span covers it (173), its span (175,
  176) and its text, from group 1 in the cell (to R2004) or in its `CELL_VALUE` list (from R2007).
  The table's row and column counts (91, 92) are read only before its first cell, where a value
  list cannot reuse them. An empty text and no value read as the same empty cell. Counts that do
  not make a whole grid leave no grid and are reported as `TABLE_GRID`; a text continued over
  several groups or a value that is not a string is left unread and reported as `TABLE_CELL_TEXT`.

### Changed

- Built on `uncad-model` 0.5.0 (a table's grid).

### Fixed

- The corpus entity counts in CAVEATS counted what model and paper space own twice -- once at
  the top level and once under the space's block record. Each entity is now counted once.

## [0.3.0] - 2026-10-04

### Added

- A MULTILEADER's content: its text (304, placed by 12 at the 171 attachment point, 41 high with
  its lines spaced by 45, in the style its 340 handle names) or its block (341, placed by 15,
  scaled by 16) -- read from the record's context data outside its leader blocks. A content the
  record says it has but does not place is not guessed. The text style and the block resolve from
  their records' handles to their names.
- The header's `$FINGERPRINTGUID` and `$VERSIONGUID` (group 2), as stated, into the model's
  `header`.
- A multileader's `line_type`, from its own override flags, type and style (the groups after
  its context data, not the like-numbered ones inside it), the MLEADERSTYLE objects' line
  type (173) and each leader line's own override (93 and 170 inside its block).

### Changed

- Built on `uncad-model` 0.4.0 (a multileader's line type and content; the header's drawing
  identifiers).

### Fixed

- A file that spells a space block in capitals (`*MODEL_SPACE`, as R13 and R14 files write it)
  no longer reads into two blocks -- an empty one under the file's spelling and another holding
  the ENTITIES section's entities. Block names are matched without regard to case, as the
  format's symbol tables match them, and a block keeps the spelling the file first wrote.

## [0.2.0] - 2026-10-02

### Added

- The drawing's `header` carries the `$INSUNITS` code the HEADER section states (`None` when
  it states none, or a value outside group 70's 16 bits).

### Changed

- A MULTILEADER is read as its leader roots (`uncad-model`'s `LeaderRoot`): each `LEADER{`
  block's lines, and its last leader line point (10, when 290 is 1) and dogleg (11 and 40,
  when 291 is 1). These were dropped before, which left a leader line of one vertex -- the
  usual case -- with nothing to draw.
- Built on `uncad-model` 0.3.0 (the drawing's `header`; a multileader's leader roots).

## [0.1.1] - 2026-09-30

### Added

- From R2013 a 3DSOLID's or a REGION's ACIS body is read from the `ACDSDATA` section, where
  those versions write it in binary (SAB) form: the entity reads as its solid with its wireframe,
  as it does up to R2010. A body record that cannot be read (a byte count that does not match its
  data, a chunk that is not hexadecimal) is reported (`ACIS_BODY_UNREADABLE`), and an entity whose
  body is missing or does not decode stays `Unknown`.

### Fixed

- An R13 or R14 3DSOLID or REGION reads its wireframe: the model now reads the one-line header
  of an ACIS SAT text from before ACIS 2.0, where every edge was skipped before.

## [0.1.0] - 2026-09-29

### Added

- Code page names besides the `ANSI_` forms are read (`GB2312`, `BIG5`, `CP932`, `CP949`,
  `CP866`, `MACINTOSH`, the `ISO-8859-` family, `US_ASCII`, `UTF8`). A byte a single-byte code
  page has no character for reads as U+FFFD and is reported (`TEXT_ENCODING`); a file from before
  R2007 that names no code page is read as ANSI_1252 and says so (`CODEPAGE_ASSUMED`).
- `read_bytes` reads binary DXF (one-byte codes up to R12, two-byte codes after) as the ASCII
  text it stands for. A group that runs past the end of the file is `ReadError::Binary`, with
  its byte offset.
- `read_bytes_with_header` and `read_str_with_header`: the drawing and, read in the same pass,
  its HEADER section as a `Header` -- every variable the file states with the groups it wrote
  for it, and `int`, `real`, `text`, `point2` and `point3` accessors. A variable written twice
  keeps its last writing and is reported as `HEADER_VARIABLE_REPEATED`.
- Reads REGION and 3DSOLID as the wireframe of their ACIS body when the record holds it (up
  to R2010), through `uncad_model::acis::wireframe`; a body stored in the `ACDSDATA` section
  (R2013 and later) is still kept as `UNKNOWN`.
- Reads a dimension's or leader's own style variables (the `ACAD` application's `DSTYLE` list in
  its extended data) into `style_overrides`; an entity without one reads as an empty list.
- Initial release. `read_bytes` (and `read_str` for text already in Unicode) reads an ASCII
  DXF file of any version into the [uncad-model](https://github.com/iyulab/uncad-model)
  entity model, with no native code behind it.
- Layers, dimension styles, multiline styles, layouts, image definitions, block definitions
  and the entities of model and paper space are read, each entity's reference ID taken from
  its handle; a pre-2007 file's strings are decoded through the code page its header declares.
- An entity type it does not interpret is kept as `UNKNOWN` under the name the file gave it,
  and a name the file does not declare as an unresolved reference: nothing is dropped.
- A file that cannot be read returns a `ReadError` naming the line. It is `#[non_exhaustive]`,
  so a later version can add a reason without breaking callers.
