# Changelog

Notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versioning follows
[Semantic Versioning](https://semver.org/). While the version is 0.x, a breaking change
bumps the minor version.

## [Unreleased]

### Added

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
