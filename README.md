# undxf

A second reader for DXF drawings into the [uncad-model](https://github.com/iyulab/uncad-model) entity model, with no native code behind it: ASCII DXF text in, a drawing out.

Built as a tool to be handed to an agent, like the other parts of the family. It contains no AI and infers nothing.

## What it does

- **Read** an ASCII DXF file (any version that is ASCII) into the model: layers, block definitions, and the entities of model and paper space, with each entity's reference ID taken from its handle so that another reader of the same file names the same entity the same way. A pre-2007 file's strings are decoded through the code page its header declares; what could not be decoded is reported in the drawing's diagnostics.
- **Keep what it does not interpret.** An entity type this crate has no interpretation for is kept as an `UNKNOWN` entity under the name the file gave it. A layer or block name the file does not declare is kept as an unresolved reference carrying that name. Nothing is silently dropped.

## What it is not

- Not a DWG reader, and not a writer.
- Not a replacement for a full CAD file library. It interprets the entity types the model carries as they are needed, and says so for the rest.
- Not an ML library. It performs no inference.

## Status

0.x, a spike. `read_bytes(&[u8])` (and `read_str(&str)` for text that is already Unicode) reads LINE, CIRCLE, ARC, ELLIPSE, LWPOLYLINE, POLYLINE (2D and 3D, with their vertices), SPLINE (its degree, knots, weights and control or fit points), TEXT, MTEXT (with its attachment point), ATTRIB, ATTDEF, INSERT (with its attributes), DIMENSION (as a reference to its anonymous block), LEADER, POINT, SOLID, TRACE, 3DFACE, HATCH (its boundary paths, fill, pattern lines and gradient), VIEWPORT (its frame on the sheet), RAY and XLINE, the LAYER table and the BLOCKS section (what the model and paper space blocks own is the drawing's own entity list); every other entity type is kept as `UNKNOWN`. What the LibreDWG test corpus says about it is in [CAVEATS.md](CAVEATS.md): 66 of its 67 DXF files read, and the same drawing saved as 2000 through 2018 reads to the same 72 entities from every version. Six of the model's golden cases read to exactly their expected models (a general part with a title block, blocks nested three deep, coincident lines, a loose-text title block, a Korean title block under code page 949, a doubled title block). A reference to a block the file does not define is kept as an unresolved reference carrying the name. Binary DXF is not read.

## License

MIT
