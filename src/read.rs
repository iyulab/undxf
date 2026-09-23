//! Sections and records: HEADER, TABLES (the LAYER table), BLOCKS and
//! ENTITIES, then the references resolved against what the tables declare.

use crate::entity::{self, Space};
use crate::pairs::{pairs, Pair, ReadError};
use std::collections::{BTreeMap, BTreeSet};
use uncad_model::model::{
    Entity, EntityCommon, EntityId, LwPolylineEntity, Point2D, Point3D, PolylineEntity,
    PolylineVertex, Ref,
};
use uncad_model::tables::{BlockRecord, DimStyleRecord, LayerRecord, Tables};
use uncad_model::{CadDatabase, ReadDiagnostics};

const MODEL_SPACE: &str = "*Model_Space";
const PAPER_SPACE: &str = "*Paper_Space";

/// Reads an ASCII DXF text into the model.
///
/// The drawing's own entities (`entities`) are what the `*Model_Space` and
/// `*Paper_Space*` blocks own, in block order: entities written inside
/// those BLOCK definitions and entities of the ENTITIES section alike
/// (the latter placed by their space flag, DXF 67). The attributes that
/// follow an INSERT (DXF 66) are attached to it and listed after it as
/// well; the vertices that follow a POLYLINE become its vertices; their
/// SEQEND is structure, not an entity, and is not kept. An entity type this
/// crate does not interpret is kept as `Unknown` under its own name.
pub fn read_str(text: &str) -> Result<CadDatabase, ReadError> {
    read_text(text, Vec::new())
}

/// Reads a DXF file's bytes into the model: the strings are decoded through
/// the code page the header declares when the file is from before R2007
/// and its bytes are not UTF-8, as UTF-8 otherwise. What could not be
/// decoded is reported in `read_diagnostics`, never dropped in silence.
pub fn read_bytes(bytes: &[u8]) -> Result<CadDatabase, ReadError> {
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err(binary());
    }
    let (text, warnings) = crate::decode::decode(bytes);
    read_text(&text, warnings)
}

fn binary() -> ReadError {
    ReadError::Structure {
        line: 1,
        detail: "binary DXF is not read; this crate reads the ASCII form".to_string(),
    }
}

fn read_text(text: &str, warnings: Vec<String>) -> Result<CadDatabase, ReadError> {
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(binary());
    }
    let pairs = pairs(text)?;
    let mut reader = Reader {
        pairs: &pairs,
        pos: 0,
        ordinal: 0,
        layers: BTreeMap::new(),
        dim_styles: BTreeMap::new(),
        mlinestyles: BTreeMap::new(),
        blocks: BTreeMap::new(),
        warnings,
    };
    reader.file()?;
    Ok(reader.finish())
}

struct Reader<'a, 'b> {
    pairs: &'b [Pair<'a>],
    pos: usize,
    /// Numbers the entities read so far, for handle-less IDs.
    ordinal: u64,
    layers: BTreeMap<String, LayerRecord>,
    dim_styles: BTreeMap<String, DimStyleRecord>,
    /// MLINESTYLE name -> each line's offset, in the style's order.
    mlinestyles: BTreeMap<String, Vec<f64>>,
    blocks: BTreeMap<String, BlockRecord>,
    /// What the decoding reported, then what each entity reported, in the
    /// order they were read.
    warnings: Vec<String>,
}

impl<'a, 'b> Reader<'a, 'b> {
    fn peek(&self) -> Option<&Pair<'a>> {
        self.pairs.get(self.pos)
    }

    fn next(&mut self) -> Option<Pair<'a>> {
        let p = self.pairs.get(self.pos).copied();
        if p.is_some() {
            self.pos += 1;
        }
        p
    }

    /// `true` when the next pair is `0/value`.
    fn at(&self, value: &str) -> bool {
        matches!(self.peek(), Some(p) if p.code == 0 && p.value == value)
    }

    fn structure(&self, detail: &str) -> ReadError {
        ReadError::Structure {
            line: self.peek().map_or(0, |p| p.line),
            detail: detail.to_string(),
        }
    }

    /// The pairs up to (not including) the next group 0.
    fn record(&mut self) -> &'b [Pair<'a>] {
        let start = self.pos;
        while let Some(p) = self.peek() {
            if p.code == 0 {
                break;
            }
            self.pos += 1;
        }
        &self.pairs[start..self.pos]
    }

    /// Skips pairs until a group 0 with `value`, consuming it.
    fn skip_to(&mut self, value: &str) -> Result<(), ReadError> {
        while let Some(p) = self.next() {
            if p.code == 0 && p.value == value {
                return Ok(());
            }
        }
        Err(self.structure(&format!("the text ends before 0/{value}")))
    }

    fn file(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends before 0/EOF"));
            };
            match (p.code, p.value) {
                (0, "EOF") => return Ok(()),
                (0, "SECTION") => {
                    let name = match self.next() {
                        Some(n) if n.code == 2 => n.value,
                        _ => return Err(self.structure("0/SECTION without a 2/name")),
                    };
                    match name {
                        "TABLES" => self.tables()?,
                        "BLOCKS" => self.blocks()?,
                        "ENTITIES" => self.entities()?,
                        "OBJECTS" => self.objects()?,
                        // HEADER, CLASSES, THUMBNAILIMAGE: nothing the model
                        // carries yet.
                        _ => self.skip_to("ENDSEC")?,
                    }
                }
                (0, other) => {
                    return Err(self.structure(&format!(
                        "expected 0/SECTION or 0/EOF at the top level, found 0/{other}"
                    )))
                }
                _ => {} // stray pairs between sections
            }
        }
    }

    fn tables(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends inside TABLES"));
            };
            match (p.code, p.value) {
                (0, "ENDSEC") => return Ok(()),
                (0, "TABLE") => {
                    let name = match self.next() {
                        Some(n) if n.code == 2 => n.value,
                        _ => return Err(self.structure("0/TABLE without a 2/name")),
                    };
                    match name {
                        "LAYER" => self.layer_table()?,
                        "DIMSTYLE" => self.dim_style_table()?,
                        _ => self.skip_to("ENDTAB")?,
                    }
                }
                _ => {}
            }
        }
    }

    /// The OBJECTS section: of its objects, the model carries the
    /// multiline styles -- each style's line offsets (49), in its order.
    fn objects(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends inside OBJECTS"));
            };
            match (p.code, p.value) {
                (0, "ENDSEC") => return Ok(()),
                (0, "MLINESTYLE") => {
                    let record = self.record();
                    let Some(name) = record.iter().find(|p| p.code == 2) else {
                        continue;
                    };
                    let offsets = record
                        .iter()
                        .filter(|p| p.code == 49)
                        .map(|p| {
                            p.value
                                .trim()
                                .parse::<f64>()
                                .map_err(|_| ReadError::BadNumber {
                                    line: p.line,
                                    code: 49,
                                    text: p.value.to_string(),
                                })
                        })
                        .collect::<Result<Vec<f64>, ReadError>>()?;
                    self.mlinestyles.insert(name.value.to_string(), offsets);
                }
                _ => {}
            }
        }
    }

    fn layer_table(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends inside the LAYER table"));
            };
            match (p.code, p.value) {
                (0, "ENDTAB") => return Ok(()),
                (0, "LAYER") => {
                    let record = self.record();
                    let Some(name) = record.iter().find(|p| p.code == 2) else {
                        continue; // the table's own header record carries no name
                    };
                    let color_index = match record.iter().find(|p| p.code == 62) {
                        Some(c) => {
                            c.value
                                .trim()
                                .parse::<i16>()
                                .map_err(|_| ReadError::BadNumber {
                                    line: c.line,
                                    code: 62,
                                    text: c.value.to_string(),
                                })?
                        }
                        None => 7,
                    };
                    self.layers.insert(
                        name.value.to_string(),
                        LayerRecord {
                            name: name.value.to_string(),
                            color_index,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    /// The DIMSTYLE table. A style variable is written only when it differs
    /// from what the application starts from, so an absent group stays
    /// `None` here rather than becoming a number this file never stated.
    fn dim_style_table(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends inside the DIMSTYLE table"));
            };
            match (p.code, p.value) {
                (0, "ENDTAB") => return Ok(()),
                (0, "DIMSTYLE") => {
                    let record = self.record();
                    let Some(name) = record.iter().find(|p| p.code == 2) else {
                        continue; // the table's own header record carries no name
                    };
                    let name = name.value.to_string();
                    let text = |code: i32| {
                        record
                            .iter()
                            .find(|p| p.code == code)
                            .map(|p| p.value.to_string())
                    };
                    let number = |code: i32| -> Result<Option<f64>, ReadError> {
                        match record.iter().find(|p| p.code == code) {
                            Some(p) => Ok(Some(p.value.trim().parse::<f64>().map_err(|_| {
                                ReadError::BadNumber {
                                    line: p.line,
                                    code,
                                    text: p.value.to_string(),
                                }
                            })?)),
                            None => Ok(None),
                        }
                    };
                    let integer = |code: i32| -> Result<Option<i32>, ReadError> {
                        match record.iter().find(|p| p.code == code) {
                            Some(p) => Ok(Some(p.value.trim().parse::<i32>().map_err(|_| {
                                ReadError::BadNumber {
                                    line: p.line,
                                    code,
                                    text: p.value.to_string(),
                                }
                            })?)),
                            None => Ok(None),
                        }
                    };
                    let style = DimStyleRecord {
                        name: name.clone(),
                        post: text(3),
                        scale: number(40)?,
                        length_factor: number(144)?,
                        tolerances: integer(71)?.map(|v| v != 0),
                        limits: integer(72)?.map(|v| v != 0),
                        tolerance_upper: number(47)?,
                        tolerance_lower: number(48)?,
                        decimal_places: integer(271)?,
                        tolerance_decimal_places: integer(272)?,
                        text_height: number(140)?,
                    };
                    self.dim_styles.insert(name, style);
                }
                _ => {}
            }
        }
    }

    fn blocks(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends inside BLOCKS"));
            };
            match (p.code, p.value) {
                (0, "ENDSEC") => return Ok(()),
                (0, "BLOCK") => {
                    let header = self.record();
                    let name = header
                        .iter()
                        .find(|p| p.code == 2)
                        .map(|p| p.value.to_string())
                        .unwrap_or_default();
                    let mut entities = Vec::new();
                    loop {
                        if self.at("ENDBLK") {
                            self.next();
                            self.record();
                            break;
                        }
                        match self.peek() {
                            Some(p) if p.code == 0 => {
                                let (entity, _) = self.group()?;
                                entities.push(entity);
                            }
                            Some(_) => {
                                self.next();
                            }
                            None => return Err(self.structure("the text ends inside a BLOCK")),
                        }
                    }
                    self.block(&name).entities.extend(entities);
                }
                _ => {}
            }
        }
    }

    fn entities(&mut self) -> Result<(), ReadError> {
        loop {
            if self.at("ENDSEC") {
                self.next();
                return Ok(());
            }
            match self.peek() {
                Some(p) if p.code == 0 => {
                    let (entity, space) = self.group()?;
                    let name = match space {
                        Space::Model => MODEL_SPACE,
                        Space::Paper => PAPER_SPACE,
                    };
                    self.block(name).entities.push(entity);
                }
                Some(_) => {
                    self.next();
                }
                None => return Err(self.structure("the text ends inside ENTITIES")),
            }
        }
    }

    /// Reads the entity at the current group 0 together with the chain
    /// that belongs to it: the ATTRIBs of an INSERT with attributes, the
    /// VERTEXes of a POLYLINE, up to and including their SEQEND.
    fn group(&mut self) -> Result<(Entity, Space), ReadError> {
        let head = self.next().expect("caller checked a group 0");
        let record = self.record();
        self.ordinal += 1;
        if head.value == "POLYLINE" {
            return self.polyline(record);
        }
        let read = entity::read(head.value, record, self.ordinal)?;
        self.warnings.extend(read.warnings);
        let mut entity = read.entity;
        if read.attribs_follow {
            let mut attribs = Vec::new();
            while self.at("ATTRIB") {
                if let Entity::Attrib(a) = self.entity()? {
                    attribs.push(a);
                }
            }
            self.seqend();
            if let Entity::Insert(i) = &mut entity {
                i.attribs = attribs;
            }
        }
        Ok((entity, read.space))
    }

    /// One entity, no chain.
    fn entity(&mut self) -> Result<Entity, ReadError> {
        let head = self.next().expect("caller checked a group 0");
        let record = self.record();
        self.ordinal += 1;
        let read = entity::read(head.value, record, self.ordinal)?;
        self.warnings.extend(read.warnings);
        Ok(read.entity)
    }

    /// Consumes a `0/SEQEND` and its record when one is next.
    fn seqend(&mut self) {
        if self.at("SEQEND") {
            self.next();
            self.record();
        }
    }

    /// A POLYLINE and its VERTEX chain. A plain 2D or 3D polyline becomes
    /// the model's polyline with its vertices. A mesh or polyface mesh
    /// (flags 16 / 64) is structure this crate does not interpret: it is
    /// kept as an `Unknown` POLYLINE whose type name says how many
    /// vertices it had, so that nothing is lost silently and nothing is
    /// guessed.
    fn polyline(&mut self, record: &[Pair<'a>]) -> Result<(Entity, Space), ReadError> {
        let read = entity::read("POLYLINE", record, self.ordinal)?;
        let flags = record
            .iter()
            .find(|p| p.code == 70)
            .and_then(|p| p.value.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let Entity::Unknown { common, .. } = read.entity else {
            unreachable!("POLYLINE is read as Unknown by entity::read")
        };
        // Each VERTEX's position, and its bulge (42 -- the segment to the
        // next vertex; absent is straight). A 3D polyline has no bulge.
        let mut vertices: Vec<(Point3D, f64)> = Vec::new();
        while self.at("VERTEX") {
            self.next();
            let vrec = self.record();
            self.ordinal += 1;
            vertices.push((entity::point3_of(vrec)?, entity::num_or(vrec, 42, 0.0)?));
        }
        self.seqend();
        let closed = flags & 1 == 1;
        let is_3d = flags & 8 == 8;
        let is_mesh = flags & (16 | 64) != 0;
        let entity = if is_mesh {
            Entity::Unknown {
                common,
                type_name: format!("POLYLINE(mesh, {} vertices)", vertices.len()),
            }
        } else if is_3d {
            Entity::Polyline3D(PolylineEntity {
                common,
                vertices: vertices.into_iter().map(|(p, _)| p).collect(),
                closed,
            })
        } else {
            Entity::Polyline2D(LwPolylineEntity {
                common,
                // The POLYLINE record's own point is a placeholder whose z is
                // the elevation of every vertex.
                elevation: entity::num_or(record, 30, 0.0)?,
                extrusion: entity::extrusion(record)?,
                vertices: vertices
                    .into_iter()
                    .map(|(p, bulge)| PolylineVertex {
                        point: Point2D { x: p.x, y: p.y },
                        bulge,
                    })
                    .collect(),
                closed,
            })
        };
        Ok((entity, read.space))
    }

    fn block(&mut self, name: &str) -> &mut BlockRecord {
        self.blocks
            .entry(name.to_string())
            .or_insert_with(|| BlockRecord {
                name: name.to_string(),
                entities: Vec::new(),
            })
    }

    /// Resolves layer and block names against the tables and entity
    /// references against the entities read, lists what the space blocks
    /// own as the drawing's own entities, and assembles the drawing.
    fn finish(self) -> CadDatabase {
        let Reader {
            layers,
            dim_styles,
            mlinestyles,
            mut blocks,
            warnings,
            ..
        } = self;
        let block_names: Vec<String> = blocks.keys().cloned().collect();
        let ids: BTreeSet<EntityId> = blocks
            .values()
            .flat_map(|b| &b.entities)
            .flat_map(|e| {
                let attribs = match e {
                    Entity::Insert(i) => i.attribs.as_slice(),
                    _ => &[],
                };
                std::iter::once(e.common().id).chain(attribs.iter().map(|a| a.common.id))
            })
            .collect();
        for block in blocks.values_mut() {
            for e in &mut block.entities {
                resolve_names(e, &layers, &block_names, &dim_styles, &mlinestyles);
                resolve_entity_refs(e, &ids);
            }
        }
        let mut entities = Vec::new();
        for (name, block) in &blocks {
            if !is_space(name) {
                continue;
            }
            for e in &block.entities {
                entities.push(e.clone());
                if let Entity::Insert(i) = e {
                    entities.extend(i.attribs.iter().cloned().map(Entity::Attrib));
                }
            }
        }
        CadDatabase {
            entities,
            tables: Tables {
                layers,
                dim_styles,
                block_records: blocks,
                mlinestyles,
            },
            read_diagnostics: ReadDiagnostics { warnings },
        }
    }
}

/// `*Model_Space` and every `*Paper_Space*` layout block.
fn is_space(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    n == MODEL_SPACE.to_ascii_uppercase() || n.starts_with(&PAPER_SPACE.to_ascii_uppercase())
}

/// A name the file wrote becomes a resolved reference when the tables
/// declare it, and stays unresolved -- carrying the name -- when they do not.
fn resolve_names(
    e: &mut Entity,
    layers: &BTreeMap<String, LayerRecord>,
    blocks: &[String],
    dim_styles: &BTreeMap<String, DimStyleRecord>,
    mlinestyles: &BTreeMap<String, Vec<f64>>,
) {
    resolve_layer(e.common_mut(), layers);
    if let Entity::MLine(m) = e {
        if let Ref::Unresolved(name) = &mut m.mlinestyle_name {
            if mlinestyles.contains_key(name.as_str()) {
                m.mlinestyle_name = Ref::Resolved(std::mem::take(name));
            }
        }
    }
    // Every entity that names a dimension style, not only the dimension: a
    // leader names one too, and resolving it for one entity and not the
    // other would make the same reference read differently depending on
    // which entity carries it.
    let style = match e {
        Entity::Dimension(d) => Some(&mut d.style_name),
        Entity::Leader(l) => Some(&mut l.style_name),
        Entity::Tolerance(t) => Some(&mut t.style_name),
        _ => None,
    };
    if let Some(style) = style {
        if let Ref::Unresolved(name) = style {
            if dim_styles.contains_key(name.as_str()) {
                *style = Ref::Resolved(std::mem::take(name));
            }
        }
    }
    let block = match e {
        Entity::Insert(i) => {
            for a in &mut i.attribs {
                resolve_layer(&mut a.common, layers);
            }
            Some(&mut i.block_name)
        }
        Entity::Dimension(d) => Some(&mut d.block_name),
        _ => None,
    };
    if let Some(block) = block {
        if let Ref::Unresolved(name) = block {
            if blocks.iter().any(|b| b == name) {
                *block = Ref::Resolved(std::mem::take(name));
            }
        }
    }
}

/// A handle the file wrote becomes a resolved reference when an entity of
/// the drawing carries it -- the ID minted the one way this reader mints
/// entity IDs, from the handle -- and stays unresolved, carrying the handle,
/// when none does.
fn resolve_entity_refs(e: &mut Entity, ids: &BTreeSet<EntityId>) {
    let Entity::Leader(l) = e else {
        return;
    };
    if let Ref::Unresolved(handle) = &l.annotation_id {
        if let Ok(value) = u64::from_str_radix(handle, 16) {
            let id = EntityId::new(value);
            if ids.contains(&id) {
                l.annotation_id = Ref::Resolved(id);
            }
        }
    }
}

fn resolve_layer(common: &mut EntityCommon, layers: &BTreeMap<String, LayerRecord>) {
    if let Ref::Unresolved(name) = &mut common.layer {
        if layers.contains_key(name.as_str()) {
            common.layer = Ref::Resolved(std::mem::take(name));
        }
    }
}
