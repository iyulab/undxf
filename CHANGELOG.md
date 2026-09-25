# Changelog

Notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versioning follows
[Semantic Versioning](https://semver.org/). While the version is 0.x, a breaking change
bumps the minor version.

## [Unreleased]

### Added

- Initial release. `read_bytes` (and `read_str` for text already in Unicode) reads an ASCII
  DXF file of any version into the [uncad-model](https://github.com/iyulab/uncad-model)
  entity model, with no native code behind it.
- Layers, dimension styles, multiline styles, layouts, image definitions, block definitions
  and the entities of model and paper space are read, each entity's reference ID taken from
  its handle; a pre-2007 file's strings are decoded through the code page its header declares.
- An entity type it does not interpret is kept as `UNKNOWN` under the name the file gave it,
  and a name the file does not declare as an unresolved reference: nothing is dropped.
  Binary DXF is not read.
