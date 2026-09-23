# Caveats

> Measured, not asserted. Every number here comes from a test that pins it (`tests/corpus_sweep.rs`, run in a tree that carries the LibreDWG test corpus beside this crate); when the reader or the corpus changes, the test fails and the number is re-measured.

## What the corpus sweep says (LibreDWG `test/test-data`, 67 DXF files, 2026-09-22)

| | Count |
|---|---|
| Files read | 66 of 67 |
| Not read | `r1.4/entities.dxf` -- a pre-R10 drawing whose first line is not a group code; that is not the pair form this crate reads, and the error names line 1 |
| Files whose bytes are not UTF-8 | 4 (`2000/TS1`, `example_2000`, `example_r13`, `example_r14`) -- all declare `ANSI_1252` and decode through it with no diagnostic |
| Reads with a diagnostic | 0 |
| Top-level entities | 1,225 |
| Entity types interpreted | LINE 49,345 · LWPOLYLINE 827 · ARC 812 · POINT 789 · INSERT 566 · DIMENSION 294 (ARC_DIMENSION included) · MTEXT 308 · SOLID 266 · ELLIPSE 201 · CIRCLE 185 · 3DFACE 182 · TEXT 116 · ATTDEF 77 · POLYLINE (2D) 66 · SPLINE 48 · ATTRIB 31 · POLYLINE (3D) 27 · LEADER 20 · TRACE 20 -- over top-level and block entities |
| Kept as `UNKNOWN` | IMAGE 70 · VIEWPORT 64 · ACAD_PROXY_ENTITY 54 · REGION 36 · WIPEOUT 32 · HATCH 28 · MULTILEADER 24 · RAY 22 · MLINE 20 · 3DSOLID 20 · TOLERANCE 18 · XLINE 18 · ACAD_TABLE 16 · SHAPE 16 · LIGHT 12 · and smaller counts of surfaces, meshes, underlays and pre-R10 REPEAT/ENDREP |

An `UNKNOWN` entity keeps its reference ID, layer, colour and type name; a consumer sees that it is there and what it is called.

## The same drawing, six versions

The corpus carries one drawing saved as DXF 2000, 2004, 2007, 2010, 2013 and 2018 (`example_*.dxf`), and another as `sample_*.dxf`. This crate reads **72** entities from every `example_*` version and **6** from every `sample_*` version -- the same counts the DWG twins of those files give through a DWG reader. A reader that goes through LibreDWG's DXF import returns **0** entities, with no error, from the 2007 and later files of this set; that silent emptiness is what this crate exists to replace on the DXF side.

Other files with a DWG twin read to the twin's count as well: `2018/Dynblocks` 121, `2000/TS1` 34, `2000/PolyLine2D` 7, `2004/material` 5, and every single-entity file under `2007/`.

`example_r14.dxf` is the one file whose counts differ: 70 here against 72 from its DWG twin. The difference is by type, and every entity read is listed with its own type -- nothing is dropped without a name:

| Type | Here (DXF) | DWG twin |
|---|---|---|
| INSERT | 12 | 10 |
| DIMENSION | 9 | 10 |
| VIEWPORT | 1 | 2 |
| XLINE | 1 | -- |
| LIGHT · MULTILEADER | -- | 1 each |
| ACAD_PROXY_ENTITY | 1 (kept as `UNKNOWN`) | 1 (as an unknown entity) |

R14 predates LIGHT and MULTILEADER, so what the DWG twin decodes under those names the R14 DXF can only carry as a proxy; the INSERT and VIEWPORT counts differ because the two readers list what the paper-space layouts own differently. Which count is right per type has not been established here -- only that the difference is visible by type rather than silent.

## What is not read yet

- **Code pages** are decoded for the `ANSI_` names the DXF reference lists (932, 936, 949, 950, 874, 1250-1258). A name outside that list is reported as `CODEPAGE_UNSUPPORTED` and the text read as UTF-8; a byte the code page has no character for reads as U+FFFD and is reported as `TEXT_ENCODING`. Bytes that are valid UTF-8 are taken as UTF-8 whatever the header says.
- **Binary DXF** is refused with an error naming line 1.
- **Pre-R10 drawings** (`r1.4`) that are not in group-code form are refused, at line 1.
- **Polyface and polygon meshes** (POLYLINE with flags 16 or 64) are kept as `UNKNOWN`, with their vertex count in the type name; their vertices are not folded into an entity the model does not have.
- **Extrusion** (DXF 210) is carried on CIRCLE, ARC, ELLIPSE, LWPOLYLINE, 2D POLYLINE, SOLID and TRACE, as the file writes it; an absent group is the world Z axis. The elevation of an LWPOLYLINE (38), of a 2D POLYLINE (its record's 30) and of a SOLID or TRACE (its first corner's 30) is carried too. A CIRCLE's and an ARC's center and a polyline's vertices stay in their own coordinate system, as the file writes them -- taking them to the world is a consumer's arithmetic.
- **A 3DFACE's invisible edges** (DXF 70, bits 1-8) are carried; an absent group is every edge visible. A missing fourth corner (13) is the third, as the reference defines it.
- **Bulges** (DXF 42) are carried on each LWPOLYLINE and 2D POLYLINE vertex; an absent 42 is a straight segment. A 42 written before an LWPOLYLINE's first vertex belongs to no vertex and is ignored. HATCH is not read at all, so its boundary bulges are not either.
- **A reference to a block the file does not define** is kept as an unresolved reference carrying the name. Another reader of this model reports such a reference as absent; which of the two the model's golden case should expect is an open question there.

## An arc-length dimension is named, not flagged

The format gives an arc-length dimension its own entity (`ARC_DIMENSION`) while still writing
group 70 on it, and that group says 5 -- the value a three-point angular dimension carries. This
reader files the dimension by the entity name and reads group 70 only for the entities named
`DIMENSION`, so an arc-length dimension does not come back as an angular one.

## What a dimension states, and what it does not

Every group a dimension carries is read from the file: 70 for the subtype, 42 for the
measurement, 1 for the text, 10 and 11 for the definition point and text middle, 13/14/15/16 for
the subtype's own points, 50 and 53 for the rotations, 3 for the style it names, 2 for the block
it is drawn with. Groups 42 and 70 have no default in the format, so a file that omits them
leaves the model saying nothing rather than zero or a subtype; groups 50 and 53 default to zero,
so an absent group is read as zero.

Group 3 names a dimension style, and this reader resolves it against the DIMSTYLE table it reads
from the same file: a dimension naming a style the file declares comes back resolved, and one
naming a style it does not comes back unresolved, carrying the name.

## A dimension style states only what it changes

The DIMSTYLE table is read for the variables the displayed text depends on -- the prefix pattern,
the scale and length factor, whether tolerances are shown and how, and the decimal places. The
format writes a style variable only when it differs from the value the application starts from,
so a variable the table omits comes back as "not stated" rather than as a number. What to use
instead is a decision about how to present the dimension, which is not this reader's to make.

A dimension's group 3 resolves against this table, so a dimension naming a style the file
declares comes back resolved, and one naming a style it does not comes back unresolved, carrying
the name.

## TOLERANCE and LEADER are not read here

The model carries what those two state -- a feature control frame's direction and style, a
leader's path, what it points at and the entity it points to. This reader does not interpret
either entity yet, so they come back under their own names as unread types, the same as the
other twenty the format defines and this reader does not. Nothing is lost silently: an unread
type keeps its name.

## A leader's arrowhead flag has no "not stated"

DXF group 71 says whether a leader draws an arrowhead. A drawing may omit
the group entirely, and the model carries the field as a plain boolean --
so "the file says no arrowhead" and "the file says nothing" arrive as the
same `false`.

This reader reports `false` for an absent group. That is a choice, not a
reading: nothing in the file supports it, and nothing supports the
opposite either. Measured on the corpus, a binary drawing always stores
the value while its text twin may leave it out, so the two readings
differ on exactly those files -- neither reader being wrong about what
its own file says.

The field beside it, the path type, is an `Option` for this same reason.
Making this one match is a change to the shared model's contract.
