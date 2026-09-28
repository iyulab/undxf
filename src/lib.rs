//! A second reader for DXF drawings into the [`uncad_model`] entity model,
//! with no native code behind it: ASCII DXF text in, a [`CadDatabase`] out.
//!
//! What it stands for is the model's own rule -- nothing read is silently
//! dropped. An entity type this crate does not interpret is kept as
//! [`uncad_model::Entity::Unknown`] under the name the file gave it; a
//! layer or block name the file does not declare is kept as an unresolved
//! reference, not blanked. A file that cannot be read at all is an error
//! that says where.
//!
//! The reference ID of an entity is its handle, so that a drawing read by
//! this crate and by another reader of the same file names its entities
//! the same way. The same bytes give the same drawing, byte for byte.
//!
//! [`read_bytes`] is the whole of it: it decodes the file's strings through
//! the code page its header declares (a pre-R2007 file) or as UTF-8, and
//! says in the drawing's diagnostics when bytes could not be decoded.
//! [`read_str`] takes text that is already Unicode. [`read_bytes_with_header`]
//! and [`read_str_with_header`] also return the file's HEADER section as a
//! [`Header`] -- every variable it states, with the groups written for it --
//! read in the same pass.

#![forbid(unsafe_code)]

mod acis;
mod binary;
mod decode;
mod entity;
mod hatch;
mod header;
mod pairs;
mod read;

pub use header::{Header, HeaderGroup};
pub use pairs::ReadError;
pub use read::{read_bytes, read_bytes_with_header, read_str, read_str_with_header};

pub use uncad_model::CadDatabase;
