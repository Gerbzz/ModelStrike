//! Editable mesh documents. Donor triangle meshes are converted into stable IDs;
//! legacy OBJ inspection and transform projects remain separate workflows.
use effectcraft_model::prim::{self, PrimParams, PrimitiveKind};
use effectcraft_model::{Model, skin_point};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;

const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: u64,
    pub position: [f64; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    pub id: u64,
    pub vertices: [u64; 3],
    pub material: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshObject {
    pub id: u64,
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub triangles: Vec<Triangle>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialSlot {
    pub id: u64,
    pub name: String,
    pub base_color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub schema_version: u32,
    pub units: String,
    pub axis: String,
    pub next_id: u64,
    pub objects: Vec<MeshObject>,
    #[serde(default)]
    pub materials: Vec<MaterialSlot>,
}

/// In-memory transaction history. Save/reopen preserves the editable document,
/// while command history is deliberately session-local.
pub struct EditSession {
    document: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl EditSession {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn new(document: Document) -> Result<Self, String> {
        document.validate()?;
        Ok(Self {
            document,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn move_vertex(&mut self, id: u64, position: [f64; 3]) -> Result<(), String> {
        let previous = self.document.clone();
        self.document.move_vertex(id, position)?;
        if self.document != previous {
            self.undo.push(previous);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(())
    }

    pub fn set_object_color(&mut self, object_id: u64, color: [f32; 4]) -> Result<(), String> {
        let previous = self.document.clone();
        self.document.set_object_color(object_id, color)?;
        if self.document != previous {
            self.undo.push(previous);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(())
    }

    pub fn inset_raise_triangle(
        &mut self,
        id: u64,
        inset_fraction: f64,
        raise_metres: f64,
    ) -> Result<(), String> {
        let previous = self.document.clone();
        self.document
            .inset_raise_triangle(id, inset_fraction, raise_metres)?;
        if self.document != previous {
            self.undo.push(previous);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        let previous = self.undo.pop().ok_or("nothing to undo")?;
        self.redo
            .push(std::mem::replace(&mut self.document, previous));
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        let next = self.redo.pop().ok_or("nothing to redo")?;
        self.undo.push(std::mem::replace(&mut self.document, next));
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    schema_version: u32,
    document_entry: String,
    document_sha256: String,
}

impl Document {
    pub fn set_object_color(&mut self, object_id: u64, color: [f32; 4]) -> Result<(), String> {
        if color
            .iter()
            .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(channel))
        {
            return Err("base color channels must be finite values from 0 to 1".into());
        }
        let mut candidate = self.clone();
        let object_index = candidate
            .objects
            .iter()
            .position(|object| object.id == object_id)
            .ok_or("mesh object ID not found")?;
        let existing = candidate.objects[object_index]
            .triangles
            .first()
            .and_then(|t| t.material)
            .filter(|id| {
                candidate.objects[object_index]
                    .triangles
                    .iter()
                    .all(|t| t.material == Some(*id))
            })
            .filter(|id| {
                !candidate.objects.iter().enumerate().any(|(index, object)| {
                    index != object_index
                        && object.triangles.iter().any(|t| t.material == Some(*id))
                })
            });
        let material_id = if let Some(id) = existing {
            candidate
                .materials
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or("mesh material ID not found")?
                .base_color = color;
            id
        } else {
            let id = candidate.next_id;
            candidate.next_id = candidate.next_id.checked_add(1).ok_or("mesh ID overflow")?;
            candidate.materials.push(MaterialSlot {
                id,
                name: "Base color".into(),
                base_color: color,
            });
            id
        };
        for triangle in &mut candidate.objects[object_index].triangles {
            triangle.material = Some(material_id);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// EffectCraft generated primitives are in pixels; ModelStrike uses metres.
    pub fn new_primitive(kind: PrimitiveKind, dimensions_metres: [f64; 3]) -> Result<Self, String> {
        if !matches!(
            kind,
            PrimitiveKind::Cube | PrimitiveKind::Plane | PrimitiveKind::Cylinder
        ) {
            return Err(
                "editable mesh currently supports cube, plane and cylinder primitives".into(),
            );
        }
        if dimensions_metres
            .iter()
            .any(|x| !x.is_finite() || *x < 0.0 || *x > 10_000.0)
            || dimensions_metres[0] == 0.0
            || dimensions_metres[1] == 0.0
            || (kind == PrimitiveKind::Plane && dimensions_metres[2] != 0.0)
            || (kind != PrimitiveKind::Plane && dimensions_metres[2] == 0.0)
        {
            return Err("primitive dimensions must be finite metres; plane depth is zero and solid dimensions are positive".into());
        }
        if kind == PrimitiveKind::Cylinder
            && (dimensions_metres[0] - dimensions_metres[2]).abs() > 1e-6
        {
            return Err("cylinder width and depth must match for a circular cross-section".into());
        }
        let size = dimensions_metres.map(|x| (x * 100.0) as f32);
        let params = PrimParams {
            size,
            radius: size[0] / 2.0,
            height: size[1],
            ..PrimParams::default()
        };
        let primitive = prim::mesh(kind, &params);
        let mut next_id = 2u64;
        let vertices: Vec<Vertex> = primitive
            .positions
            .iter()
            .enumerate()
            .map(|(index, position)| {
                let id = next_id;
                next_id += 1;
                Vertex {
                    id,
                    position: position.map(|value| {
                        ((f64::from(value) / 100.0) * 1_000_000.0).round() / 1_000_000.0
                    }),
                    normal: primitive
                        .normals
                        .get(index)
                        .copied()
                        .unwrap_or([0.0, 1.0, 0.0]),
                    uv: primitive.uvs.get(index).copied().unwrap_or([0.0, 0.0]),
                }
            })
            .collect();
        let triangles = primitive
            .indices
            .chunks_exact(3)
            .map(|corner| {
                let id = next_id;
                next_id += 1;
                Triangle {
                    id,
                    vertices: [
                        vertices[corner[0] as usize].id,
                        vertices[corner[1] as usize].id,
                        vertices[corner[2] as usize].id,
                    ],
                    material: None,
                }
            })
            .collect();
        let document = Self {
            schema_version: 1,
            units: "metre".into(),
            axis: "Y_up".into(),
            next_id,
            objects: vec![MeshObject {
                id: 1,
                name: kind.label().into(),
                vertices,
                triangles,
            }],
            materials: Vec::new(),
        };
        document.validate()?;
        Ok(document)
    }

    /// Import a donor-supported OBJ, glTF or GLB as a static editable mesh.
    /// OBJ is unitless; `unit_scale` is metres per source unit. glTF is metres.
    pub fn import_model(path: &Path, unit_scale: f64) -> Result<(Self, Vec<String>), String> {
        if !unit_scale.is_finite() || unit_scale <= 0.0 || unit_scale > 1_000_000.0 {
            return Err("import scale must be positive finite metres per source unit".into());
        }
        let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["obj", "gltf", "glb"].contains(&ext.as_str()) {
            return Err("select an OBJ, glTF or GLB model".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err("source model is too large".into());
        }
        if ext == "obj" {
            crate::inspect_obj_bytes(&bytes)?;
        }
        let parent = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let resolve = |uri: &str| -> Option<Vec<u8>> {
            let relative = Path::new(uri);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return None;
            }
            let resolved = parent.join(relative).canonicalize().ok()?;
            if !resolved.starts_with(&parent) || resolved.metadata().ok()?.len() > MAX_ARCHIVE_BYTES
            {
                return None;
            }
            std::fs::read(resolved).ok()
        };
        let model = effectcraft_model::load(&path.to_string_lossy(), &bytes, &resolve)
            .map_err(|e| e.to_string())?;
        let scale = if ext == "obj" { unit_scale } else { 1.0 };
        let mut warnings = Vec::new();
        if ext != "obj" && unit_scale != 1.0 {
            warnings.push("glTF uses metres; OBJ scale option was ignored".into());
        }
        if !model.animations.is_empty() {
            warnings.push("animations imported at rest pose only".into());
        }
        if !model.skins.is_empty() {
            warnings.push("skinned mesh imported at rest pose only".into());
        }
        if !model.textures.is_empty() {
            warnings.push("textures are not stored in editable mesh documents yet".into());
        }
        if !model.cameras.is_empty() || !model.lights.is_empty() {
            warnings.push("cameras and lights are omitted".into());
        }
        let document = Self::from_static_model(&model, scale)?;
        Ok((document, warnings))
    }

    fn from_static_model(model: &Model, unit_scale: f64) -> Result<Self, String> {
        let mut next_id = 1u64;
        let materials: Vec<MaterialSlot> = model
            .materials
            .iter()
            .map(|source| {
                let id = next_id;
                next_id += 1;
                MaterialSlot {
                    id,
                    name: source.name.clone(),
                    base_color: source.base_color,
                }
            })
            .collect();
        let mut objects = Vec::new();
        for instance in model.instances(None, 0.0) {
            for (part_index, primitive) in instance.mesh.primitives.iter().enumerate() {
                if primitive.indices.len() % 3 != 0 {
                    return Err("source primitive has incomplete triangles".into());
                }
                let object_id = next_id;
                next_id += 1;
                let mut vertices = Vec::with_capacity(primitive.positions.len());
                for (index, &position) in primitive.positions.iter().enumerate() {
                    let p = skin_point(&instance, primitive, index, position);
                    let normal = primitive
                        .normals
                        .get(index)
                        .copied()
                        .unwrap_or([0.0, 1.0, 0.0]);
                    let n = instance.world.apply_vec(effectcraft_model::vec3(
                        normal[0] as f64,
                        normal[1] as f64,
                        normal[2] as f64,
                    ));
                    let magnitude = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
                    let normal = if magnitude > 1e-12 {
                        [
                            (n.x / magnitude) as f32,
                            (n.y / magnitude) as f32,
                            (n.z / magnitude) as f32,
                        ]
                    } else {
                        normal
                    };
                    vertices.push(Vertex {
                        id: next_id,
                        position: [p[0] * unit_scale, p[1] * unit_scale, p[2] * unit_scale],
                        normal,
                        uv: primitive.uvs.get(index).copied().unwrap_or([0.0, 0.0]),
                    });
                    next_id += 1;
                }
                let mut triangles = Vec::new();
                for corner in primitive.indices.chunks_exact(3) {
                    let ids = corner
                        .iter()
                        .map(|&index| {
                            vertices
                                .get(index as usize)
                                .map(|v| v.id)
                                .ok_or("source triangle index outside vertices")
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    triangles.push(Triangle {
                        id: next_id,
                        vertices: [ids[0], ids[1], ids[2]],
                        material: primitive
                            .material
                            .and_then(|index| materials.get(index).map(|m| m.id)),
                    });
                    next_id += 1;
                }
                objects.push(MeshObject {
                    id: object_id,
                    name: format!(
                        "{} {}",
                        if instance.mesh.name.is_empty() {
                            "Imported mesh"
                        } else {
                            &instance.mesh.name
                        },
                        part_index + 1
                    ),
                    vertices,
                    triangles,
                });
            }
        }
        let document = Self {
            schema_version: 1,
            units: "metre".into(),
            axis: "Y_up".into(),
            next_id,
            objects,
            materials,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.units != "metre" || self.axis != "Y_up" {
            return Err("unsupported mesh document schema, units or axis".into());
        }
        if self.objects.is_empty() || self.objects.len() > 10_000 {
            return Err("mesh document needs 1 to 10000 objects".into());
        }
        let mut ids = HashSet::new();
        let mut max_id = 0u64;
        let mut material_ids = HashSet::new();
        for material in &self.materials {
            if material.name.len() > 256
                || material
                    .base_color
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                || !ids.insert(material.id)
            {
                return Err("material needs a unique ID and finite base colour".into());
            }
            material_ids.insert(material.id);
            max_id = max_id.max(material.id);
        }
        for object in &self.objects {
            if object.name.trim().is_empty() || object.name.len() > 256 || !ids.insert(object.id) {
                return Err("object needs a valid name and unique ID".into());
            }
            max_id = max_id.max(object.id);
            if object.vertices.len() > 1_000_000 || object.triangles.len() > 2_000_000 {
                return Err("mesh exceeds document geometry limits".into());
            }
            let mut vertex_positions = HashMap::new();
            for vertex in &object.vertices {
                if !ids.insert(vertex.id) {
                    return Err("duplicate mesh ID".into());
                }
                vertex_positions.insert(vertex.id, vertex.position);
                max_id = max_id.max(vertex.id);
                if vertex
                    .position
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
                    || vertex.normal.iter().any(|v| !v.is_finite())
                    || vertex.uv.iter().any(|v| !v.is_finite())
                {
                    return Err("vertex has non-finite or out-of-range geometry".into());
                }
            }
            for triangle in &object.triangles {
                if !ids.insert(triangle.id) {
                    return Err("duplicate mesh ID".into());
                }
                max_id = max_id.max(triangle.id);
                if triangle
                    .material
                    .is_some_and(|id| !material_ids.contains(&id))
                {
                    return Err("triangle references a missing material".into());
                }
                if triangle
                    .vertices
                    .iter()
                    .any(|id| !vertex_positions.contains_key(id))
                    || triangle.vertices[0] == triangle.vertices[1]
                    || triangle.vertices[0] == triangle.vertices[2]
                    || triangle.vertices[1] == triangle.vertices[2]
                {
                    return Err("triangle has missing or repeated vertices".into());
                }
                let p = triangle.vertices.map(|id| vertex_positions[&id]);
                let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
                let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
                let cross = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                if cross.iter().map(|x| x * x).sum::<f64>() < 1e-20 {
                    return Err("triangle has zero area".into());
                }
            }
        }
        if self.next_id <= max_id {
            return Err("next mesh ID must exceed all used IDs".into());
        }
        Ok(())
    }

    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let mut positions = self
            .objects
            .iter()
            .flat_map(|object| object.vertices.iter().map(|v| v.position));
        let first = positions.next()?;
        let (mut min, mut max) = (first, first);
        for p in positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        Some((min, max))
    }

    pub fn triangle_count(&self) -> usize {
        self.objects
            .iter()
            .map(|object| object.triangles.len())
            .sum()
    }

    /// Commit a vertex move only if the complete resulting document is valid.
    pub fn move_vertex(&mut self, id: u64, position: [f64; 3]) -> Result<(), String> {
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
        {
            return Err("vertex position must be finite and within mesh limits".into());
        }
        let mut candidate = self.clone();
        let vertex = candidate
            .objects
            .iter_mut()
            .flat_map(|object| &mut object.vertices)
            .find(|vertex| vertex.id == id)
            .ok_or("vertex ID not found")?;
        vertex.position = position;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Add a raised triangular detail, keeping the selected face ID as its cap.
    /// All geometry and IDs are built in a candidate document before commit.
    pub fn inset_raise_triangle(
        &mut self,
        id: u64,
        inset_fraction: f64,
        raise_metres: f64,
    ) -> Result<(), String> {
        self.validate()?;
        if !inset_fraction.is_finite() || !(0.02..=0.45).contains(&inset_fraction) {
            return Err("inset fraction must be finite and from 0.02 to 0.45".into());
        }
        if !raise_metres.is_finite() || raise_metres <= 0.0 || raise_metres > 1000.0 {
            return Err("raise must be finite, positive and at most 1000 metres".into());
        }
        let mut candidate = self.clone();
        let (object_index, triangle_index) = candidate
            .objects
            .iter()
            .enumerate()
            .find_map(|(object_index, object)| {
                object
                    .triangles
                    .iter()
                    .position(|triangle| triangle.id == id)
                    .map(|triangle_index| (object_index, triangle_index))
            })
            .ok_or("triangle face ID not found")?;
        let object = &candidate.objects[object_index];
        if object.vertices.len() + 6 > 1_000_000 || object.triangles.len() + 12 > 2_000_000 {
            return Err("face edit would exceed document geometry limits".into());
        }
        let face = object.triangles[triangle_index].clone();
        let corners: [Vertex; 3] = face.vertices.map(|vertex_id| {
            object
                .vertices
                .iter()
                .find(|vertex| vertex.id == vertex_id)
                .expect("validated triangle references a vertex")
                .clone()
        });
        let a = corners[0].position;
        let b = corners[1].position;
        let c = corners[2].position;
        let edge_a = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let edge_b = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cross = [
            edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
            edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
            edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
        ];
        let length = cross.iter().map(|value| value * value).sum::<f64>().sqrt();
        if !length.is_finite() || length < 1e-10 {
            return Err("selected face has insufficient area".into());
        }
        let normal = cross.map(|value| value / length);
        let centre: [f64; 3] = std::array::from_fn(|axis| (a[axis] + b[axis] + c[axis]) / 3.0);
        let uv_centre = std::array::from_fn::<_, 2, _>(|axis| {
            (f64::from(corners[0].uv[axis])
                + f64::from(corners[1].uv[axis])
                + f64::from(corners[2].uv[axis]))
                / 3.0
        });
        let end_id = candidate
            .next_id
            .checked_add(18)
            .ok_or("mesh ID overflow")?;
        let start_id = candidate.next_id;
        candidate.next_id = end_id;
        let base_ids = [start_id, start_id + 1, start_id + 2];
        let cap_ids = [start_id + 3, start_id + 4, start_id + 5];
        let mut added_vertices = Vec::with_capacity(6);
        for (index, corner) in corners.iter().enumerate() {
            let inner = std::array::from_fn(|axis| {
                corner.position[axis] * (1.0 - inset_fraction) + centre[axis] * inset_fraction
            });
            let uv = std::array::from_fn(|axis| {
                (f64::from(corner.uv[axis]) * (1.0 - inset_fraction)
                    + uv_centre[axis] * inset_fraction) as f32
            });
            added_vertices.push(Vertex {
                id: base_ids[index],
                position: inner,
                normal: normal.map(|value| value as f32),
                uv,
            });
            added_vertices.push(Vertex {
                id: cap_ids[index],
                position: std::array::from_fn(|axis| inner[axis] + normal[axis] * raise_metres),
                normal: normal.map(|value| value as f32),
                uv,
            });
        }
        let object = &mut candidate.objects[object_index];
        object.vertices.extend(added_vertices);
        object.triangles[triangle_index].vertices = cap_ids;
        let mut next_triangle_id = start_id + 6;
        for index in 0..3 {
            let next = (index + 1) % 3;
            for vertices in [
                [face.vertices[index], face.vertices[next], base_ids[next]],
                [face.vertices[index], base_ids[next], base_ids[index]],
                [base_ids[index], base_ids[next], cap_ids[next]],
                [base_ids[index], cap_ids[next], cap_ids[index]],
            ] {
                object.triangles.push(Triangle {
                    id: next_triangle_id,
                    vertices,
                    material: face.material,
                });
                next_triangle_id += 1;
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Export the current triangulated mesh to an OBJ with UV0 and normals.
    pub fn export_obj_new(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut pending = tempfile::Builder::new()
            .suffix(".obj")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        writeln!(
            pending,
            "# ModelStrike triangulated mesh; units: metres; Y up"
        )
        .map_err(|e| e.to_string())?;
        let mut offset = 0usize;
        let mut normal_index = 0usize;
        for object in &self.objects {
            writeln!(pending, "o {}", object.name.replace(['\r', '\n'], " "))
                .map_err(|e| e.to_string())?;
            let mut indices = HashMap::new();
            for (index, vertex) in object.vertices.iter().enumerate() {
                indices.insert(vertex.id, offset + index + 1);
                writeln!(
                    pending,
                    "v {} {} {}",
                    vertex.position[0], vertex.position[1], vertex.position[2]
                )
                .map_err(|e| e.to_string())?;
            }
            for vertex in &object.vertices {
                writeln!(pending, "vt {} {}", vertex.uv[0], vertex.uv[1])
                    .map_err(|e| e.to_string())?;
            }
            let lookup: HashMap<u64, &Vertex> = object
                .vertices
                .iter()
                .map(|vertex| (vertex.id, vertex))
                .collect();
            for triangle in &object.triangles {
                let corners = triangle.vertices.map(|id| indices[&id]);
                let p = triangle.vertices.map(|id| lookup[&id].position);
                let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
                let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
                let cross = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                let length = cross.iter().map(|v| v * v).sum::<f64>().sqrt();
                normal_index += 1;
                writeln!(
                    pending,
                    "vn {} {} {}",
                    cross[0] / length,
                    cross[1] / length,
                    cross[2] / length
                )
                .map_err(|e| e.to_string())?;
                writeln!(
                    pending,
                    "f {0}/{0}/{3} {1}/{1}/{3} {2}/{2}/{3}",
                    corners[0], corners[1], corners[2], normal_index
                )
                .map_err(|e| e.to_string())?;
            }
            offset += object.vertices.len();
        }
        pending.as_file().sync_all().map_err(|e| e.to_string())?;
        crate::inspect_obj(pending.path())?;
        pending
            .persist_noclobber(path)
            .map_err(|e| e.error.to_string())?;
        Ok(())
    }

    /// Export a static, triangulated glTF 2.0 binary. Face normals are flat;
    /// texture images, animation and non-base-colour PBR fields are not stored.
    pub fn export_glb_new(&self, path: &Path) -> Result<Vec<String>, String> {
        self.validate()?;
        let bytes = self.glb_bytes()?;
        let model = effectcraft_model::load("export.glb", &bytes, &|_| None)
            .map_err(|e| format!("GLB self-check failed: {e}"))?;
        if model.triangle_count() != self.triangle_count() {
            return Err("GLB self-check triangle count mismatch".into());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut pending = tempfile::Builder::new()
            .suffix(".glb")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        pending
            .write_all(&bytes)
            .and_then(|_| pending.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        pending
            .persist_noclobber(path)
            .map_err(|e| e.error.to_string())?;
        Ok(vec!["triangulated output with flat face normals; texture images, animation and extended PBR data are not exported".into()])
    }

    fn glb_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bin = Vec::new();
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        let mut meshes = Vec::new();
        let mut nodes = Vec::new();
        let material_lookup: HashMap<u64, usize> = self
            .materials
            .iter()
            .enumerate()
            .map(|(index, m)| (m.id, index))
            .collect();
        for object in &self.objects {
            let vertex_lookup: HashMap<u64, &Vertex> =
                object.vertices.iter().map(|v| (v.id, v)).collect();
            let mut groups: BTreeMap<Option<u64>, Vec<&Triangle>> = BTreeMap::new();
            for triangle in &object.triangles {
                groups.entry(triangle.material).or_default().push(triangle);
            }
            let mut primitives = Vec::new();
            for (material_id, triangles) in groups {
                let mut positions = Vec::new();
                let mut normals = Vec::new();
                let mut uvs = Vec::new();
                let mut indices = Vec::new();
                let mut min = [f32::INFINITY; 3];
                let mut max = [f32::NEG_INFINITY; 3];
                for triangle in triangles {
                    let corners = triangle.vertices.map(|id| vertex_lookup[&id]);
                    let p = corners.map(|v| v.position);
                    let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
                    let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
                    let cross = [
                        a[1] * b[2] - a[2] * b[1],
                        a[2] * b[0] - a[0] * b[2],
                        a[0] * b[1] - a[1] * b[0],
                    ];
                    let length = cross.iter().map(|v| v * v).sum::<f64>().sqrt();
                    let normal = cross.map(|v| (v / length) as f32);
                    let converted = p.map(|point| point.map(|value| value as f32));
                    if converted.iter().flatten().any(|value| !value.is_finite()) {
                        return Err("GLB coordinate exceeds float32 range".into());
                    }
                    let q = converted.map(|point| point.map(f64::from));
                    let qa = [q[1][0] - q[0][0], q[1][1] - q[0][1], q[1][2] - q[0][2]];
                    let qb = [q[2][0] - q[0][0], q[2][1] - q[0][1], q[2][2] - q[0][2]];
                    let quantized_cross = [
                        qa[1] * qb[2] - qa[2] * qb[1],
                        qa[2] * qb[0] - qa[0] * qb[2],
                        qa[0] * qb[1] - qa[1] * qb[0],
                    ];
                    let quantized_length = quantized_cross
                        .iter()
                        .map(|value| value * value)
                        .sum::<f64>()
                        .sqrt();
                    let aligned = cross
                        .iter()
                        .zip(quantized_cross)
                        .map(|(source, written)| *source * written)
                        .sum::<f64>()
                        > 0.0;
                    let shortest_edge = [(0, 1), (1, 2), (2, 0)]
                        .map(|(first, second)| {
                            (0..3)
                                .map(|axis| (p[first][axis] - p[second][axis]).powi(2))
                                .sum::<f64>()
                                .sqrt()
                        })
                        .into_iter()
                        .fold(f64::INFINITY, f64::min);
                    let max_position_error = (0..3)
                        .map(|corner| {
                            (0..3)
                                .map(|axis| (p[corner][axis] - q[corner][axis]).powi(2))
                                .sum::<f64>()
                                .sqrt()
                        })
                        .fold(0.0, f64::max);
                    if !aligned
                        || !(0.99..=1.01).contains(&(quantized_length / length))
                        || max_position_error > shortest_edge * 0.01
                    {
                        return Err("GLB export would collapse or distort a triangle beyond 1% at float32 precision; move geometry nearer the origin or enlarge it".into());
                    }
                    for corner in corners {
                        let index = u32::try_from(indices.len())
                            .map_err(|_| "GLB vertex count exceeds u32".to_string())?;
                        indices.push(index);
                        for axis in 0..3 {
                            let value = corner.position[axis] as f32;
                            if !value.is_finite() {
                                return Err("GLB coordinate exceeds f32 range".into());
                            }
                            positions.push(value);
                            min[axis] = min[axis].min(value);
                            max[axis] = max[axis].max(value);
                            normals.push(normal[axis]);
                        }
                        uvs.extend(corner.uv);
                    }
                }
                let position_accessor = push_f32_accessor(
                    &mut bin,
                    &mut views,
                    &mut accessors,
                    &positions,
                    3,
                    Some((min, max)),
                )?;
                let normal_accessor =
                    push_f32_accessor(&mut bin, &mut views, &mut accessors, &normals, 3, None)?;
                let uv_accessor =
                    push_f32_accessor(&mut bin, &mut views, &mut accessors, &uvs, 2, None)?;
                let index_accessor =
                    push_u32_accessor(&mut bin, &mut views, &mut accessors, &indices)?;
                let mut primitive = serde_json::json!({
                    "attributes": {"POSITION": position_accessor, "NORMAL": normal_accessor, "TEXCOORD_0": uv_accessor},
                    "indices": index_accessor,
                    "mode": 4
                });
                if let Some(id) = material_id {
                    primitive["material"] = serde_json::json!(material_lookup[&id]);
                }
                primitives.push(primitive);
            }
            if !primitives.is_empty() {
                let mesh_index = meshes.len();
                meshes.push(serde_json::json!({"name": object.name, "primitives": primitives}));
                nodes.push(serde_json::json!({"name": object.name, "mesh": mesh_index}));
            }
        }
        if meshes.is_empty() {
            return Err("GLB export needs at least one face".into());
        }
        let materials: Vec<_> = self.materials.iter().map(|m| serde_json::json!({
            "name": m.name,
            "pbrMetallicRoughness": {"baseColorFactor": m.base_color, "metallicFactor": 0.0, "roughnessFactor": 0.8}
        })).collect();
        let scene_nodes: Vec<_> = (0..nodes.len()).collect();
        let root = serde_json::json!({
            "asset": {"version": "2.0", "generator": "ModelStrike"},
            "scene": 0,
            "scenes": [{"nodes": scene_nodes}],
            "nodes": nodes,
            "meshes": meshes,
            "materials": materials,
            "buffers": [{"byteLength": bin.len()}],
            "bufferViews": views,
            "accessors": accessors
        });
        let mut json = serde_json::to_vec(&root).map_err(|e| e.to_string())?;
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let total = 12usize
            .checked_add(8 + json.len())
            .and_then(|v| v.checked_add(8 + bin.len()))
            .ok_or("GLB size overflow")?;
        let total =
            u32::try_from(total).map_err(|_| "GLB exceeds 4 GiB format limit".to_string())?;
        let mut out = Vec::with_capacity(total as usize);
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend(total.to_le_bytes());
        out.extend(
            u32::try_from(json.len())
                .map_err(|_| "GLB JSON too large")?
                .to_le_bytes(),
        );
        out.extend(b"JSON");
        out.extend(json);
        out.extend(
            u32::try_from(bin.len())
                .map_err(|_| "GLB BIN too large")?
                .to_le_bytes(),
        );
        out.extend(b"BIN\0");
        out.extend(bin);
        Ok(out)
    }

    fn write_pending_archive(&self, path: &Path) -> Result<tempfile::NamedTempFile, String> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut pending = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("mesh document is too large".into());
        }
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let manifest = Manifest {
            format: "ModelStrike editable mesh".into(),
            schema_version: 1,
            document_entry: "document.json".into(),
            document_sha256: digest,
        };
        {
            let mut archive = zip::ZipWriter::new(pending.as_file_mut());
            let options = zip::write::SimpleFileOptions::default();
            archive
                .start_file("manifest.json", options)
                .map_err(|e| e.to_string())?;
            archive
                .write_all(&serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            archive
                .start_file("document.json", options)
                .map_err(|e| e.to_string())?;
            archive.write_all(&bytes).map_err(|e| e.to_string())?;
            archive.finish().map_err(|e| e.to_string())?;
        }
        pending.as_file().sync_all().map_err(|e| e.to_string())?;
        Self::load(pending.path())?;
        Ok(pending)
    }

    pub fn save_new(&self, path: &Path) -> Result<(), String> {
        let pending = self.write_pending_archive(path)?;
        pending
            .persist_noclobber(path)
            .map_err(|e| e.error.to_string())?;
        Ok(())
    }

    pub fn archive_sha256(path: &Path) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err("mesh archive is too large".into());
        }
        Ok(format!("{:x}", Sha256::digest(&bytes)))
    }

    pub fn previous_path(path: &Path) -> std::path::PathBuf {
        path.with_extension("previous.mstrike")
    }

    /// Replace only the exact archive version that was opened or last saved.
    /// A sibling previous revision is secured before the new validated archive replaces it.
    pub fn save_replace(&self, path: &Path, expected_sha256: &str) -> Result<(), String> {
        Self::load(path)?;
        if Self::archive_sha256(path)? != expected_sha256 {
            return Err("mesh document changed on disk; reopen before saving".into());
        }
        let pending = self.write_pending_archive(path)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut backup = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        std::io::copy(
            &mut std::fs::File::open(path).map_err(|e| e.to_string())?,
            backup.as_file_mut(),
        )
        .map_err(|e| e.to_string())?;
        backup.as_file().sync_all().map_err(|e| e.to_string())?;
        backup
            .persist(Self::previous_path(path))
            .map_err(|e| e.error.to_string())?;
        pending.persist(path).map_err(|e| e.error.to_string())?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_ARCHIVE_BYTES {
            return Err("mesh archive is too large".into());
        }
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        if archive.len() != 2 {
            return Err("mesh archive must have exactly two entries".into());
        }
        let mut manifest_bytes = Vec::new();
        {
            let mut entry = archive
                .by_name("manifest.json")
                .map_err(|e| e.to_string())?;
            if entry.size() > 64 * 1024 {
                return Err("mesh manifest is too large".into());
            }
            entry
                .read_to_end(&mut manifest_bytes)
                .map_err(|e| e.to_string())?;
        }
        let manifest: Manifest =
            serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
        if manifest.format != "ModelStrike editable mesh"
            || manifest.schema_version != 1
            || manifest.document_entry != "document.json"
        {
            return Err("unsupported mesh manifest".into());
        }
        let mut bytes = Vec::new();
        {
            let mut entry = archive
                .by_name("document.json")
                .map_err(|e| e.to_string())?;
            if entry.size() > MAX_DOCUMENT_BYTES {
                return Err("mesh document is too large".into());
            }
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        }
        if format!("{:x}", Sha256::digest(&bytes)) != manifest.document_sha256 {
            return Err("mesh document checksum mismatch".into());
        }
        let document: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        document.validate()?;
        Ok(document)
    }
}

fn push_view(
    bin: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    bytes: &[u8],
    target: u32,
) -> Result<usize, String> {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let offset = bin.len();
    let end = offset
        .checked_add(bytes.len())
        .ok_or("GLB buffer size overflow")?;
    if end > 256 * 1024 * 1024 {
        return Err("GLB export exceeds 256 MiB limit".into());
    }
    bin.extend_from_slice(bytes);
    let index = views.len();
    views.push(serde_json::json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target}));
    Ok(index)
}

fn push_f32_accessor(
    bin: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    values: &[f32],
    width: usize,
    bounds: Option<([f32; 3], [f32; 3])>,
) -> Result<usize, String> {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    let view = push_view(bin, views, &bytes, 34962)?;
    let mut accessor = serde_json::json!({"bufferView": view, "componentType": 5126,
        "count": values.len() / width, "type": if width == 2 { "VEC2" } else { "VEC3" }});
    if let Some((min, max)) = bounds {
        accessor["min"] = serde_json::json!(min);
        accessor["max"] = serde_json::json!(max);
    }
    let index = accessors.len();
    accessors.push(accessor);
    Ok(index)
}

fn push_u32_accessor(
    bin: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    values: &[u32],
) -> Result<usize, String> {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    let view = push_view(bin, views, &bytes, 34963)?;
    let index = accessors.len();
    accessors.push(
        serde_json::json!({"bufferView": view, "componentType": 5125,
        "count": values.len(), "type": "SCALAR"}),
    );
    Ok(index)
}
