//! Sections and records: HEADER, TABLES, BLOCKS, ENTITIES and OBJECTS, then
//! the references resolved against what the tables declare.

use crate::decode::string;
use crate::entity::{self, Space};
use crate::pairs::{pairs, Pair, ReadError};
use std::collections::{BTreeMap, BTreeSet};
use uncad_model::model::{
    Entity, EntityCommon, EntityId, LwPolylineEntity, Point2D, Point3D, PolylineEntity,
    PolylineVertex, Ref, Solid3DEntity,
};
use uncad_model::tables::{
    AngularUnitFormat, BlockRecord, DimStyleRecord, FractionFormat, LayerRecord, LayoutRecord,
    LinearUnitFormat, PlotPaperUnits, PlotRotation, PlotSettings, Tables,
};
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
        layer_handles: BTreeMap::new(),
        linetypes: BTreeSet::new(),
        text_styles: BTreeSet::new(),
        block_record_handles: BTreeMap::new(),
        dim_styles: BTreeMap::new(),
        mlinestyles: BTreeMap::new(),
        layouts: BTreeMap::new(),
        blocks: BTreeMap::new(),
        warnings,
        version: None,
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
    /// A LAYER record's handle (upper-case hex) -> its name: how a viewport
    /// names the layers it freezes.
    layer_handles: BTreeMap<String, String>,
    /// The LTYPE table's names, which a layer's linetype resolves against.
    linetypes: BTreeSet<String>,
    /// The STYLE table's names, which a text's style resolves against.
    text_styles: BTreeSet<String>,
    /// A BLOCK_RECORD's handle -> its name: how a layout names its block.
    block_record_handles: BTreeMap<String, String>,
    dim_styles: BTreeMap<String, DimStyleRecord>,
    /// MLINESTYLE name -> each line's offset, in the style's order.
    mlinestyles: BTreeMap<String, Vec<f64>>,
    /// LAYOUT name -> record, its block still named by handle.
    layouts: BTreeMap<String, LayoutRecord>,
    blocks: BTreeMap<String, BlockRecord>,
    /// What the decoding reported, then what each entity reported, in the
    /// order they were read.
    warnings: Vec<String>,
    /// `$ACADVER` from the HEADER section (`AC1015` for R2000), when the file
    /// has one. It says which variables the file's format has at all.
    version: Option<String>,
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
                        "HEADER" => self.header()?,
                        "TABLES" => self.tables()?,
                        "BLOCKS" => self.blocks()?,
                        "ENTITIES" => self.entities()?,
                        "OBJECTS" => self.objects()?,
                        // CLASSES, THUMBNAILIMAGE: nothing the model carries
                        // yet.
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

    /// The HEADER section: only `$ACADVER` is kept.
    fn header(&mut self) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure("the text ends inside HEADER"));
            };
            match (p.code, p.value) {
                (0, "ENDSEC") => return Ok(()),
                (9, "$ACADVER") => {
                    if let Some(v) = self.peek().filter(|v| v.code != 9 && v.code != 0) {
                        self.version = Some(v.value.trim().to_string());
                    }
                }
                _ => {}
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
                        "LTYPE" | "STYLE" | "BLOCK_RECORD" => self.named_table(name)?,
                        _ => self.skip_to("ENDTAB")?,
                    }
                }
                _ => {}
            }
        }
    }

    /// The OBJECTS section: of its objects, the model carries the
    /// multiline styles -- each style's line offsets (49), in its order --
    /// and the layouts.
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
                    self.mlinestyles.insert(string(name.value), offsets);
                }
                (0, "LAYOUT") => {
                    let record = self.record();
                    if let Some(layout) = layout(record)? {
                        self.layouts.insert(layout.name.clone(), layout);
                    }
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
                    let Some(layer) = layer(record)? else {
                        continue; // the table's own header record carries no name
                    };
                    if let Some(handle) = record.iter().find(|p| p.code == 5) {
                        self.layer_handles
                            .insert(handle.value.trim().to_ascii_uppercase(), layer.name.clone());
                    }
                    self.layers.insert(layer.name.clone(), layer);
                }
                _ => {}
            }
        }
    }

    /// A table the model carries only the names of -- LTYPE and STYLE, whose
    /// entries a layer's linetype and a text's style resolve against -- or
    /// the handles of -- BLOCK_RECORD, through which a layout names its
    /// block.
    fn named_table(&mut self, table: &str) -> Result<(), ReadError> {
        loop {
            let Some(p) = self.next() else {
                return Err(self.structure(&format!("the text ends inside the {table} table")));
            };
            match (p.code, p.value) {
                (0, "ENDTAB") => return Ok(()),
                (0, entry) if entry == table => {
                    let record = self.record();
                    let Some(name) = record.iter().find(|p| p.code == 2) else {
                        continue; // the table's own header record carries no name
                    };
                    let name = string(name.value);
                    match table {
                        "LTYPE" => {
                            self.linetypes.insert(name);
                        }
                        "STYLE" => {
                            self.text_styles.insert(name);
                        }
                        _ => {
                            if let Some(handle) = record.iter().find(|p| p.code == 5) {
                                self.block_record_handles
                                    .insert(handle.value.trim().to_ascii_uppercase(), name);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// The DIMSTYLE table. A style variable is written only when it differs
    /// from the value the application starts from. Where that value is the
    /// same whichever template a drawing started from (a tolerance of 0, a
    /// factor of 1, a switch that is off, the decimal unit format), an
    /// absent group is that value. Where the templates start differently
    /// (text height, decimal places, zero suppression), an absent group
    /// stays `None` rather than becoming a number this file never stated.
    ///
    /// Three of the variables (the linear unit format, the fraction format
    /// and an angle's decimal places) came with R2000; a file of an earlier
    /// version -- or one whose header does not say -- cannot have written
    /// them, so there an absent group is `None` too.
    fn dim_style_table(&mut self) -> Result<(), ReadError> {
        let r2000 = self.version.as_deref().is_some_and(|v| v >= "AC1015");
        let since_r2000 = |v: Option<i32>, starting: i32| v.or(r2000.then_some(starting));
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
                    let name = string(name.value);
                    let text = |code: i32| {
                        record
                            .iter()
                            .find(|p| p.code == code)
                            .map(|p| string(p.value))
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
                    // Before R2000 one variable, DIMUNIT (270), said both.
                    let dimunit = integer(270)?.map_or((None, None), dimunit);
                    let style = DimStyleRecord {
                        name: name.clone(),
                        post: Some(text(3).unwrap_or_default()),
                        scale: Some(number(40)?.unwrap_or(1.0)),
                        length_factor: Some(number(144)?.unwrap_or(1.0)),
                        tolerances: Some(integer(71)?.is_some_and(|v| v != 0)),
                        limits: Some(integer(72)?.is_some_and(|v| v != 0)),
                        tolerance_upper: Some(number(47)?.unwrap_or(0.0)),
                        tolerance_lower: Some(number(48)?.unwrap_or(0.0)),
                        decimal_places: integer(271)?,
                        tolerance_decimal_places: integer(272)?,
                        text_height: number(140)?,
                        arrow_size: number(41)?,
                        linear_unit_format: if r2000 {
                            since_r2000(integer(277)?, 2).and_then(|v| match v {
                                1 => Some(LinearUnitFormat::Scientific),
                                2 => Some(LinearUnitFormat::Decimal),
                                3 => Some(LinearUnitFormat::Engineering),
                                4 => Some(LinearUnitFormat::Architectural),
                                5 => Some(LinearUnitFormat::Fractional),
                                6 => Some(LinearUnitFormat::WindowsDesktop),
                                _ => None,
                            })
                        } else {
                            dimunit.0
                        },
                        zero_suppression: integer(78)?,
                        rounding: Some(number(45)?.unwrap_or(0.0)),
                        angular_unit_format: Some(integer(275)?.unwrap_or(0)).and_then(
                            |v| match v {
                                0 => Some(AngularUnitFormat::DecimalDegrees),
                                1 => Some(AngularUnitFormat::DegreesMinutesSeconds),
                                2 => Some(AngularUnitFormat::Gradians),
                                3 => Some(AngularUnitFormat::Radians),
                                4 => Some(AngularUnitFormat::SurveyorsUnits),
                                _ => None,
                            },
                        ),
                        angular_decimal_places: since_r2000(integer(179)?, 0),
                        fraction_format: if r2000 {
                            since_r2000(integer(276)?, 0).and_then(|v| match v {
                                0 => Some(FractionFormat::Horizontal),
                                1 => Some(FractionFormat::Diagonal),
                                2 => Some(FractionFormat::NotStacked),
                                _ => None,
                            })
                        } else {
                            dimunit.1
                        },
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
                        .map(|p| string(p.value))
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
    /// the model's polyline with its vertices, and a polygon mesh (flag 16)
    /// the wireframe of its grid. A polyface mesh (flag 64) is structure
    /// this crate does not interpret: it is kept as an `Unknown` POLYLINE
    /// whose type name says how many vertices it had, so that nothing is
    /// lost silently and nothing is guessed.
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
        // The widths a vertex that states none of its own takes: the
        // POLYLINE record's own 40 and 41.
        let default_start = entity::num_or(record, 40, 0.0)?;
        let default_end = entity::num_or(record, 41, 0.0)?;
        // Each VERTEX's position, its bulge (42 -- the segment to the next
        // vertex; absent is straight) and its segment's widths (40, 41). A
        // 3D polyline has no bulge.
        let mut vertices: Vec<(Point3D, PolylineVertex)> = Vec::new();
        while self.at("VERTEX") {
            self.next();
            let vrec = self.record();
            self.ordinal += 1;
            let at = entity::point3_of(vrec)?;
            vertices.push((
                at,
                PolylineVertex {
                    point: Point2D { x: at.x, y: at.y },
                    bulge: entity::num_or(vrec, 42, 0.0)?,
                    start_width: entity::num_or(vrec, 40, default_start)?,
                    end_width: entity::num_or(vrec, 41, default_end)?,
                },
            ));
        }
        self.seqend();
        let closed = flags & 1 == 1;
        let entity = if flags & 16 == 16 {
            let count = |code: i32| {
                entity::num_or(record, code, 0.0).map(|v| usize::try_from(v as i64).unwrap_or(0))
            };
            let (wireframe_edges, skipped_edges) = mesh_wireframe(
                &vertices.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
                count(71)?,
                count(72)?,
                closed,
                flags & 32 == 32,
            );
            Entity::PolylineMesh(Solid3DEntity {
                common,
                wireframe_edges,
                skipped_edges,
            })
        } else if flags & 64 == 64 {
            Entity::Unknown {
                common,
                type_name: format!("POLYLINE(mesh, {} vertices)", vertices.len()),
            }
        } else if flags & 8 == 8 {
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
                vertices: vertices.into_iter().map(|(_, v)| v).collect(),
                closed,
                // A 2D POLYLINE's widths are its vertices'.
                const_width: 0.0,
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
            mut layers,
            layer_handles,
            linetypes,
            text_styles,
            block_record_handles,
            dim_styles,
            mlinestyles,
            mut layouts,
            mut blocks,
            warnings,
            ..
        } = self;
        let block_names: Vec<String> = blocks.keys().cloned().collect();
        for layer in layers.values_mut() {
            resolve_name(&mut layer.linetype, |n| linetypes.contains(n));
        }
        for layout in layouts.values_mut() {
            resolve_block_record(&mut layout.block_name, &block_record_handles, &block_names);
        }
        let names = Names {
            layers: &layers,
            layer_handles: &layer_handles,
            blocks: &block_names,
            dim_styles: &dim_styles,
            mlinestyles: &mlinestyles,
            text_styles: &text_styles,
        };
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
                resolve_names(e, &names);
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
                layouts,
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

/// The names and handles the tables declare, which the entities' references
/// resolve against.
struct Names<'n> {
    layers: &'n BTreeMap<String, LayerRecord>,
    layer_handles: &'n BTreeMap<String, String>,
    blocks: &'n [String],
    dim_styles: &'n BTreeMap<String, DimStyleRecord>,
    mlinestyles: &'n BTreeMap<String, Vec<f64>>,
    text_styles: &'n BTreeSet<String>,
}

/// A name the file wrote becomes a resolved reference when `declared` says
/// the tables declare it, and stays unresolved -- carrying the name -- when
/// they do not.
fn resolve_name(reference: &mut Ref<String>, declared: impl Fn(&str) -> bool) {
    if let Ref::Unresolved(name) = reference {
        if declared(name) {
            *reference = Ref::Resolved(std::mem::take(name));
        }
    }
}

/// A text's style (DXF 7), resolved against the STYLE table. The file names
/// no style when the text is in the reference's default one, the style named
/// `STANDARD`; that entry is looked up without regard to case, the way the
/// table names it, and a drawing that declares none leaves the reference
/// absent.
fn resolve_text_style(style: &mut Ref<String>, styles: &BTreeSet<String>) {
    match style {
        Ref::Absent => {
            if let Some(standard) = styles.iter().find(|s| s.eq_ignore_ascii_case("STANDARD")) {
                *style = Ref::Resolved(standard.clone());
            }
        }
        _ => resolve_name(style, |n| styles.contains(n)),
    }
}

/// A layout's block, named by its BLOCK_RECORD's handle: the record's name
/// when the table declares the handle -- resolved when a block of that name
/// was read, unresolved and carrying it when none was -- and the handle
/// itself, unresolved, when the table does not.
fn resolve_block_record(
    block: &mut Ref<String>,
    handles: &BTreeMap<String, String>,
    blocks: &[String],
) {
    if let Ref::Unresolved(handle) = block {
        if let Some(name) = handles.get(handle.as_str()) {
            *block = if blocks.iter().any(|b| b == name) {
                Ref::Resolved(name.clone())
            } else {
                Ref::Unresolved(name.clone())
            };
        }
    }
}

/// A name the file wrote becomes a resolved reference when the tables
/// declare it, and stays unresolved -- carrying the name -- when they do not.
fn resolve_names(e: &mut Entity, names: &Names<'_>) {
    resolve_layer(e.common_mut(), names.layers);
    match e {
        Entity::MLine(m) => resolve_name(&mut m.mlinestyle_name, |n| {
            names.mlinestyles.contains_key(n)
        }),
        Entity::Text(t) => resolve_text_style(&mut t.style_name, names.text_styles),
        Entity::Attrib(a) => resolve_text_style(&mut a.style_name, names.text_styles),
        Entity::Attdef(a) => resolve_text_style(&mut a.style_name, names.text_styles),
        Entity::MText(m) => resolve_text_style(&mut m.style_name, names.text_styles),
        // A viewport names the layers it freezes by their records' handles.
        Entity::Viewport(v) => {
            for layer in &mut v.frozen_layers {
                if let Ref::Unresolved(handle) = layer {
                    if let Some(name) = names.layer_handles.get(handle.as_str()) {
                        *layer = Ref::Resolved(name.clone());
                    }
                }
            }
        }
        _ => {}
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
        resolve_name(style, |n| names.dim_styles.contains_key(n));
    }
    let block = match e {
        Entity::Insert(i) => {
            for a in &mut i.attribs {
                resolve_layer(&mut a.common, names.layers);
                resolve_text_style(&mut a.style_name, names.text_styles);
            }
            Some(&mut i.block_name)
        }
        Entity::Dimension(d) => Some(&mut d.block_name),
        _ => None,
    };
    if let Some(block) = block {
        resolve_name(block, |n| names.blocks.iter().any(|b| b == n));
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

/// The first value of group `code` in `record`, parsed.
fn value<T: std::str::FromStr>(record: &[Pair<'_>], code: i32) -> Result<Option<T>, ReadError> {
    match record.iter().find(|p| p.code == code) {
        Some(p) => p
            .value
            .trim()
            .parse::<T>()
            .map(Some)
            .map_err(|_| ReadError::BadNumber {
                line: p.line,
                code,
                text: p.value.to_string(),
            }),
        None => Ok(None),
    }
}

/// A LAYER table entry, or `None` for a record without a name (the table's
/// own header record).
///
/// A DXF says a layer is off with the sign of its colour (62), and states
/// group 70 in its own bit layout: 1 frozen, 4 locked. The plot flag (290)
/// and the lineweight (370) exist from R2000 on and are `None` where the
/// file does not write them. The linetype (6) is carried by name and
/// resolved against the LTYPE table once the tables are in.
fn layer(record: &[Pair<'_>]) -> Result<Option<LayerRecord>, ReadError> {
    let Some(name) = record.iter().find(|p| p.code == 2) else {
        return Ok(None);
    };
    let color_index = value::<i16>(record, 62)?.unwrap_or(7);
    let flags = value::<i64>(record, 70)?.unwrap_or(0);
    Ok(Some(LayerRecord {
        name: string(name.value),
        color_index,
        off: color_index < 0,
        frozen: flags & 1 != 0,
        locked: flags & 4 != 0,
        plot: value::<i64>(record, 290)?.map(|v| v != 0),
        lineweight: value::<i16>(record, 370)?,
        linetype: match record.iter().find(|p| p.code == 6) {
            Some(p) if !p.value.is_empty() => Ref::Unresolved(string(p.value)),
            _ => Ref::Absent,
        },
    }))
}

/// A LAYOUT object, or `None` for one without a name. The object has two
/// parts that reuse each other's group codes: its plot settings
/// (`AcDbPlotSettings`) and the layout itself (`AcDbLayout`), which starts at
/// that marker. The block it shows is named by its BLOCK_RECORD's handle
/// (the layout part's 330; the object's first 330 is its owner) and
/// resolved once the tables are in. Every distance of the plot settings is
/// in millimetres, as the reference states them.
fn layout(record: &[Pair<'_>]) -> Result<Option<LayoutRecord>, ReadError> {
    let split = record
        .iter()
        .position(|p| p.code == 100 && p.value.trim() == "AcDbLayout")
        .unwrap_or(record.len());
    let (plot, own) = record.split_at(split);
    let Some(name) = own.iter().find(|p| p.code == 1) else {
        return Ok(None);
    };
    let number = |part: &[Pair<'_>], code: i32| -> Result<f64, ReadError> {
        Ok(value::<f64>(part, code)?.unwrap_or(0.0))
    };
    let point = |part: &[Pair<'_>], x: i32| -> Result<Point2D, ReadError> {
        Ok(Point2D {
            x: number(part, x)?,
            y: number(part, x + 10)?,
        })
    };
    let block_name = match own.iter().find(|p| p.code == 330).map(|p| p.value.trim()) {
        Some(handle) if !handle.is_empty() && handle != "0" => {
            Ref::Unresolved(handle.to_ascii_uppercase())
        }
        _ => Ref::Absent,
    };
    Ok(Some(LayoutRecord {
        name: string(name.value),
        tab_order: value::<i32>(own, 71)?.unwrap_or(0),
        block_name,
        limits_min: point(own, 10)?,
        limits_max: point(own, 11)?,
        plot_settings: PlotSettings {
            paper_name: plot
                .iter()
                .find(|p| p.code == 4)
                .map(|p| string(p.value))
                .unwrap_or_default(),
            paper_width: number(plot, 44)?,
            paper_height: number(plot, 45)?,
            margin_left: number(plot, 40)?,
            margin_bottom: number(plot, 41)?,
            margin_right: number(plot, 42)?,
            margin_top: number(plot, 43)?,
            plot_origin: Point2D {
                x: number(plot, 46)?,
                y: number(plot, 47)?,
            },
            paper_units: value::<i64>(plot, 72)?.and_then(|v| match v {
                0 => Some(PlotPaperUnits::Inches),
                1 => Some(PlotPaperUnits::Millimeters),
                2 => Some(PlotPaperUnits::Pixels),
                _ => None,
            }),
            rotation: value::<i64>(plot, 73)?.and_then(|v| match v {
                0 => Some(PlotRotation::Unrotated),
                1 => Some(PlotRotation::Counterclockwise90),
                2 => Some(PlotRotation::UpsideDown),
                3 => Some(PlotRotation::Clockwise90),
                _ => None,
            }),
            scale_numerator: number(plot, 142)?,
            scale_denominator: number(plot, 143)?,
        },
    }))
}

/// A polygon mesh's grid lines, in the order the model states for
/// `Entity::PolylineMesh`: `m` rows of `n` vertices, stored row by row (vertex
/// `i * n + j` is row `i`, column `j`); first the edges `(i, j)-(i + 1, j)` row
/// by row, then `(i, j)-(i, j + 1)` row by row, the closing edges included
/// where the grid wraps (`closed_m` -- group 70 bit 1 -- and `closed_n`, bit
/// 32).
///
/// Returns the edges and how many the grid's definition promises but could
/// not be drawn: unless exactly `m * n` vertices were written, no grid shape
/// is guessed -- a smoothed mesh writes spline control points beside the
/// approximated ones -- and every edge the definition implies is reported
/// as skipped, so the mesh reads as "not read" rather than as an empty one.
fn mesh_wireframe(
    positions: &[Point3D],
    m: usize,
    n: usize,
    closed_m: bool,
    closed_n: bool,
) -> (Vec<[Point3D; 2]>, usize) {
    let rows_joined = if closed_m { m } else { m.saturating_sub(1) };
    let columns_joined = if closed_n { n } else { n.saturating_sub(1) };
    let edge_count = rows_joined
        .saturating_mul(n)
        .saturating_add(m.saturating_mul(columns_joined));
    if positions.is_empty() || positions.len() != m.saturating_mul(n) {
        return (Vec::new(), edge_count);
    }
    let at = |i: usize, j: usize| positions[i * n + j];
    let mut edges = Vec::with_capacity(edge_count);
    for i in 0..rows_joined {
        for j in 0..n {
            edges.push([at(i, j), at((i + 1) % m, j)]);
        }
    }
    for i in 0..m {
        for j in 0..columns_joined {
            edges.push([at(i, j), at(i, (j + 1) % n)]);
        }
    }
    (edges, 0)
}

/// What a pre-R2000 dimension style's DIMUNIT (DXF 270) says: the linear
/// unit format, and -- where the value says it -- the fraction format. 4
/// and 5 are the architectural and fractional formats with stacked
/// fractions, which the variable does not say how to stack; 6 and 7 are the
/// same without stacking. R2000 split the variable into DIMLUNIT and
/// DIMFRAC.
fn dimunit(value: i32) -> (Option<LinearUnitFormat>, Option<FractionFormat>) {
    match value {
        1 => (Some(LinearUnitFormat::Scientific), None),
        2 => (Some(LinearUnitFormat::Decimal), None),
        3 => (Some(LinearUnitFormat::Engineering), None),
        4 => (Some(LinearUnitFormat::Architectural), None),
        5 => (Some(LinearUnitFormat::Fractional), None),
        6 => (
            Some(LinearUnitFormat::Architectural),
            Some(FractionFormat::NotStacked),
        ),
        7 => (
            Some(LinearUnitFormat::Fractional),
            Some(FractionFormat::NotStacked),
        ),
        8 => (Some(LinearUnitFormat::WindowsDesktop), None),
        _ => (None, None),
    }
}
