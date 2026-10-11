use crate::editor::{Document, EditSession};
use effectcraft_model::prim::PrimitiveKind;
use eframe::egui;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

enum PendingAction {
    New(PrimitiveKind, [f64; 3]),
    Open(PathBuf),
    Import(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionMode {
    Vertex,
    Face,
}

enum Picked {
    Vertex(u64),
    Face(u64),
}

pub(super) struct MeshPanel {
    session: Option<EditSession>,
    saved: Option<Document>,
    path: Option<PathBuf>,
    archive_hash: Option<String>,
    selected_vertex: Option<u64>,
    selected_face: Option<u64>,
    selection_mode: SelectionMode,
    edit_position: [f64; 3],
    inset_fraction: f64,
    raise_metres: f64,
    material_color: [f32; 4],
    kind: PrimitiveKind,
    dimensions: [f64; 3],
    status: String,
    error: Option<String>,
    pending: Option<PendingAction>,
}

impl Default for MeshPanel {
    fn default() -> Self {
        Self {
            session: None,
            saved: None,
            path: None,
            archive_hash: None,
            selected_vertex: None,
            selected_face: None,
            selection_mode: SelectionMode::Vertex,
            edit_position: [0.0; 3],
            inset_fraction: 0.2,
            raise_metres: 0.1,
            material_color: [0.75, 0.75, 0.75, 1.0],
            kind: PrimitiveKind::Cube,
            dimensions: [1.0; 3],
            status: "Create a primitive or import a model to begin.".into(),
            error: None,
            pending: None,
        }
    }
}

impl MeshPanel {
    fn dirty(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| Some(session.document()) != self.saved.as_ref())
    }

    fn install(&mut self, document: Document, path: Option<PathBuf>) -> Result<(), String> {
        let hash = path.as_deref().map(Document::archive_sha256).transpose()?;
        let saved = path.as_ref().map(|_| document.clone());
        self.session = Some(EditSession::new(document)?);
        self.saved = saved;
        self.path = path;
        self.archive_hash = hash;
        self.selected_vertex = None;
        self.selected_face = None;
        self.sync_material_color();
        self.error = None;
        Ok(())
    }

    fn new_primitive(&mut self, kind: PrimitiveKind, dimensions: [f64; 3]) -> Result<(), String> {
        let document = Document::new_primitive(kind, dimensions)?;
        self.install(document, None)?;
        self.status = format!("New {} mesh in metres", kind.label());
        Ok(())
    }

    pub(super) fn open_document(&mut self, path: &Path) -> Result<(), String> {
        let document = Document::load(path)?;
        self.install(document, Some(path.to_path_buf()))?;
        self.status = format!("Opened {}", path.display());
        Ok(())
    }

    fn import_model(&mut self, path: &Path) -> Result<(), String> {
        let (document, warnings) = Document::import_model(path, 1.0)?;
        self.install(document, None)?;
        self.status = if warnings.is_empty() {
            format!("Imported {}", path.display())
        } else {
            format!(
                "Imported {}. Losses: {}",
                path.display(),
                warnings.join("; ")
            )
        };
        Ok(())
    }

    pub(super) fn select_vertex(&mut self, id: u64) -> Result<(), String> {
        let vertex = self
            .session
            .as_ref()
            .ok_or("no mesh document")?
            .document()
            .objects
            .iter()
            .flat_map(|object| &object.vertices)
            .find(|vertex| vertex.id == id)
            .ok_or("vertex ID not found")?;
        self.selected_vertex = Some(id);
        self.selected_face = None;
        self.selection_mode = SelectionMode::Vertex;
        self.edit_position = vertex.position;
        self.sync_material_color();
        Ok(())
    }

    pub(super) fn select_face(&mut self, id: u64) -> Result<(), String> {
        let found = self
            .session
            .as_ref()
            .ok_or("no mesh document")?
            .document()
            .objects
            .iter()
            .any(|object| object.triangles.iter().any(|triangle| triangle.id == id));
        if !found {
            return Err("triangle face ID not found".into());
        }
        self.selected_face = Some(id);
        self.selected_vertex = None;
        self.selection_mode = SelectionMode::Face;
        self.sync_material_color();
        Ok(())
    }

    fn selected_object_id(&self) -> Option<u64> {
        let document = self.session.as_ref()?.document();
        self.selected_vertex
            .and_then(|id| {
                document
                    .objects
                    .iter()
                    .find(|object| object.vertices.iter().any(|v| v.id == id))
            })
            .or_else(|| {
                self.selected_face.and_then(|id| {
                    document
                        .objects
                        .iter()
                        .find(|object| object.triangles.iter().any(|t| t.id == id))
                })
            })
            .or_else(|| document.objects.first())
            .map(|object| object.id)
    }

    fn sync_material_color(&mut self) {
        self.material_color = [0.75, 0.75, 0.75, 1.0];
        let Some(document) = self.session.as_ref().map(EditSession::document) else {
            return;
        };
        let Some(object_id) = self.selected_object_id() else {
            return;
        };
        let Some(material_id) = document
            .objects
            .iter()
            .find(|o| o.id == object_id)
            .and_then(|o| o.triangles.first())
            .and_then(|t| t.material)
        else {
            return;
        };
        if let Some(material) = document.materials.iter().find(|m| m.id == material_id) {
            self.material_color = material.base_color;
        }
    }

    fn apply_material_color(&mut self) -> Result<(), String> {
        let object_id = self.selected_object_id().ok_or("no mesh object")?;
        self.session
            .as_mut()
            .ok_or("no mesh document")?
            .set_object_color(object_id, self.material_color)?;
        self.status = format!("Applied base color to object {object_id}; GLB export keeps it");
        Ok(())
    }

    fn apply_vertex_position(&mut self) -> Result<(), String> {
        let id = self.selected_vertex.ok_or("select a vertex first")?;
        self.session
            .as_mut()
            .ok_or("no mesh document")?
            .move_vertex(id, self.edit_position)?;
        self.status = format!("Moved vertex {id}; undo is available");
        Ok(())
    }

    fn apply_face_edit(&mut self) -> Result<(), String> {
        let id = self.selected_face.ok_or("select a face first")?;
        self.session
            .as_mut()
            .ok_or("no mesh document")?
            .inset_raise_triangle(id, self.inset_fraction, self.raise_metres)?;
        self.status = format!("Inset and raised face {id}; one undo step available");
        Ok(())
    }

    fn sync_selected(&mut self) {
        if let Some(id) = self.selected_vertex {
            if self.select_vertex(id).is_err() {
                self.selected_vertex = None;
            }
        }
        if let Some(id) = self.selected_face {
            if self.select_face(id).is_err() {
                self.selected_face = None;
            }
        }
    }

    fn undo(&mut self) -> Result<(), String> {
        self.session.as_mut().ok_or("no mesh document")?.undo()?;
        self.sync_selected();
        self.sync_material_color();
        self.status = "Undid mesh edit".into();
        Ok(())
    }

    fn redo(&mut self) -> Result<(), String> {
        self.session.as_mut().ok_or("no mesh document")?.redo()?;
        self.sync_selected();
        self.sync_material_color();
        self.status = "Redid mesh edit".into();
        Ok(())
    }

    fn save(&mut self, path: &Path) -> Result<(), String> {
        let document = self
            .session
            .as_ref()
            .ok_or("create or import a mesh first")?
            .document();
        if self.path.as_deref() == Some(path) {
            document.save_replace(
                path,
                self.archive_hash
                    .as_deref()
                    .ok_or("saved file hash is missing")?,
            )?;
        } else {
            document.save_new(path)?;
        }
        self.archive_hash = Some(Document::archive_sha256(path)?);
        self.saved = Some(document.clone());
        self.path = Some(path.to_path_buf());
        self.status = format!("Saved {}", path.display());
        self.error = None;
        Ok(())
    }

    fn export_glb(&mut self, path: &Path) -> Result<(), String> {
        let report = self
            .session
            .as_ref()
            .ok_or("create or import a mesh first")?
            .document()
            .export_glb_new(path)?;
        self.status = format!("Exported {}. {}", path.display(), report.join("; "));
        self.error = None;
        Ok(())
    }

    fn export_obj(&mut self, path: &Path) -> Result<(), String> {
        self.session
            .as_ref()
            .ok_or("create or import a mesh first")?
            .document()
            .export_obj_new(path)?;
        self.status = format!(
            "Exported triangulated OBJ to {}. Materials are omitted.",
            path.display()
        );
        self.error = None;
        Ok(())
    }

    fn request(&mut self, action: PendingAction) -> Result<(), String> {
        if self.dirty() {
            self.pending = Some(action);
            self.status =
                "Unsaved mesh edits: save, discard or cancel before opening another mesh".into();
            Ok(())
        } else {
            self.perform(action)
        }
    }

    fn perform(&mut self, action: PendingAction) -> Result<(), String> {
        match action {
            PendingAction::New(kind, dimensions) => self.new_primitive(kind, dimensions),
            PendingAction::Open(path) => self.open_document(&path),
            PendingAction::Import(path) => self.import_model(&path),
        }
    }

    fn choose_save_path(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("ModelStrike editable mesh", &["mstrike"])
            .set_file_name("untitled.mstrike")
            .save_file()
    }

    fn save_current(&mut self) -> Result<(), String> {
        let path = match &self.path {
            Some(path) => path.clone(),
            None => self.choose_save_path().ok_or("save canceled")?,
        };
        self.save(&path)
    }

    pub(super) fn show(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("mesh_toolbar").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal_wrapped(|ui| {
                ui.strong("MESH");
                ui.separator();
                ui.menu_button("New mesh", |ui| {
                    ui.strong("Primitive in metres");
                    ui.horizontal(|ui| {
                        for kind in [
                            PrimitiveKind::Cube,
                            PrimitiveKind::Plane,
                            PrimitiveKind::Cylinder,
                        ] {
                            if ui
                                .selectable_label(self.kind == kind, kind.label())
                                .clicked()
                            {
                                self.kind = kind;
                                self.dimensions[2] = if kind == PrimitiveKind::Plane {
                                    0.0
                                } else {
                                    self.dimensions[0]
                                };
                            }
                        }
                    });
                    for axis in 0..3 {
                        ui.horizontal(|ui| {
                            ui.label(["Width", "Height", "Depth"][axis]);
                            ui.add_enabled(
                                axis != 2 || self.kind != PrimitiveKind::Plane,
                                egui::DragValue::new(&mut self.dimensions[axis]).speed(0.05),
                            );
                        });
                    }
                    if ui.button("Create new document").clicked() {
                        let result = self.request(PendingAction::New(self.kind, self.dimensions));
                        self.error = result.err();
                        ui.close_menu();
                    }
                });
                if ui.button("Open .mstrike").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Editable mesh", &["mstrike"])
                        .pick_file()
                    {
                        let result = self.request(PendingAction::Open(path));
                        self.error = result.err();
                    }
                }
                if ui.button("Import OBJ / GLB").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("3D source", &["obj", "gltf", "glb"])
                        .pick_file()
                    {
                        let result = self.request(PendingAction::Import(path));
                        self.error = result.err();
                    }
                }
                let has_doc = self.session.is_some();
                if ui
                    .add_enabled(has_doc, egui::Button::new("Save  Ctrl+S"))
                    .clicked()
                {
                    let result = self.save_current();
                    self.error = result.err();
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new("Save as…"))
                    .clicked()
                {
                    if let Some(path) = self.choose_save_path() {
                        let result = self.save(&path);
                        self.error = result.err();
                    }
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new("Export GLB"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("glTF binary", &["glb"])
                        .set_file_name("prop.glb")
                        .save_file()
                    {
                        let result = self.export_glb(&path);
                        self.error = result.err();
                    }
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new("Export OBJ"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Wavefront OBJ", &["obj"])
                        .set_file_name("prop.obj")
                        .save_file()
                    {
                        let result = self.export_obj(&path);
                        self.error = result.err();
                    }
                }
            });
            ui.add_space(5.0);
        });

        egui::SidePanel::left("mesh_hierarchy").default_width(220.0).resizable(true).show(ctx, |ui| {
            ui.heading("Objects & elements");
            if let Some(session) = &self.session {
                let document = session.document();
                ui.label(format!("{} object(s) · {} triangles", document.objects.len(), document.triangle_count()));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.selection_mode, SelectionMode::Vertex, "Vertices");
                    ui.selectable_value(&mut self.selection_mode, SelectionMode::Face, "Faces");
                });
                ui.small("Select an element here or in the preview; edit it in Properties.");
                let mut select = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for object in &document.objects {
                        ui.separator();
                        ui.strong(&object.name);
                        ui.small(format!("{} vertices · {} triangles", object.vertices.len(), object.triangles.len()));
                        match self.selection_mode {
                            SelectionMode::Vertex => {
                                for vertex in object.vertices.iter().take(5000) {
                                    if ui.selectable_label(self.selected_vertex == Some(vertex.id), format!("Vertex {}", vertex.id))
                                        .on_hover_text(format!("{:.3}, {:.3}, {:.3} m", vertex.position[0], vertex.position[1], vertex.position[2])).clicked() {
                                        select = Some(Picked::Vertex(vertex.id));
                                    }
                                }
                                if object.vertices.len() > 5000 { ui.small("First 5000 vertices shown; preview selection remains available."); }
                            }
                            SelectionMode::Face => {
                                for face in object.triangles.iter().take(5000) {
                                    if ui.selectable_label(self.selected_face == Some(face.id), format!("Face {}", face.id)).clicked() {
                                        select = Some(Picked::Face(face.id));
                                    }
                                }
                                if object.triangles.len() > 5000 { ui.small("First 5000 faces shown; preview selection remains available."); }
                            }
                        }
                    }
                });
                if let Some(picked) = select {
                    let result = match picked {
                        Picked::Vertex(id) => self.select_vertex(id),
                        Picked::Face(id) => self.select_face(id),
                    };
                    self.error = result.err();
                }
            } else {
                ui.label("No editable mesh open");
            }
        });

        egui::SidePanel::right("mesh_inspector")
            .default_width(255.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Properties");
                if let Some(session) = &self.session {
                    let can_undo = session.can_undo();
                    let can_redo = session.can_redo();
                    let document = session.document();
                    ui.label(if self.dirty() {
                        "● Unsaved changes"
                    } else {
                        "Saved"
                    });
                    if let Some(path) = &self.path {
                        ui.small(path.file_name().unwrap_or_default().to_string_lossy())
                            .on_hover_text(path.display().to_string());
                    }
                    if let Some((min, max)) = document.bounds() {
                        ui.separator();
                        ui.strong("Bounds · metres");
                        for axis in 0..3 {
                            ui.label(format!(
                                "{}  {:.3} … {:.3}",
                                ["X", "Y", "Z"][axis],
                                min[axis],
                                max[axis]
                            ));
                        }
                    }
                    ui.separator();
                    if self.selection_mode == SelectionMode::Vertex {
                        if let Some(id) = self.selected_vertex {
                            ui.strong(format!("Vertex {id}"));
                            for axis in 0..3 {
                                ui.horizontal(|ui| {
                                    ui.label(["X m", "Y m", "Z m"][axis]);
                                    ui.add(
                                        egui::DragValue::new(&mut self.edit_position[axis])
                                            .speed(0.01),
                                    );
                                });
                            }
                            if ui.button("Apply vertex position").clicked() {
                                let result = self.apply_vertex_position();
                                self.error = result.err();
                            }
                        } else {
                            ui.label("Select a vertex to edit its position.");
                        }
                    } else if let Some(id) = self.selected_face {
                        ui.strong(format!("Face {id}"));
                        ui.small("Inset the triangle and raise a cap along its winding normal.");
                        ui.horizontal(|ui| {
                            ui.label("Inset fraction");
                            ui.add(
                                egui::DragValue::new(&mut self.inset_fraction)
                                    .speed(0.01)
                                    .range(0.02..=0.45),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Raise m");
                            ui.add(
                                egui::DragValue::new(&mut self.raise_metres)
                                    .speed(0.01)
                                    .range(0.001..=1000.0),
                            );
                        });
                        if ui.button("Apply inset and raise").clicked() {
                            let result = self.apply_face_edit();
                            self.error = result.err();
                        }
                        ui.small("No collision check against other mesh surfaces.");
                    } else {
                        ui.label("Select a face to inset and raise it.");
                    }
                    ui.separator();
                    ui.strong("Object base color");
                    ui.small("RGBA factors saved in the mesh and exported to GLB.");
                    let swatch = egui::Color32::from_rgb(
                        (self.material_color[0] * 255.0) as u8,
                        (self.material_color[1] * 255.0) as u8,
                        (self.material_color[2] * 255.0) as u8,
                    );
                    ui.colored_label(swatch, "Color preview");
                    for axis in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(["R", "G", "B", "A"][axis]);
                            ui.add(
                                egui::DragValue::new(&mut self.material_color[axis])
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            );
                        });
                    }
                    if ui.button("Apply base color").clicked() {
                        let result = self.apply_material_color();
                        self.error = result.err();
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(can_undo, egui::Button::new("Undo  Ctrl+Z"))
                            .clicked()
                        {
                            let result = self.undo();
                            self.error = result.err();
                        }
                        if ui
                            .add_enabled(can_redo, egui::Button::new("Redo  Ctrl+Y"))
                            .clicked()
                        {
                            let result = self.redo();
                            self.error = result.err();
                        }
                    });
                } else {
                    ui.strong("New primitive");
                    ui.horizontal(|ui| {
                        for kind in [
                            PrimitiveKind::Cube,
                            PrimitiveKind::Plane,
                            PrimitiveKind::Cylinder,
                        ] {
                            if ui
                                .selectable_label(self.kind == kind, kind.label())
                                .clicked()
                            {
                                self.kind = kind;
                                self.dimensions[2] = if kind == PrimitiveKind::Plane {
                                    0.0
                                } else {
                                    self.dimensions[0]
                                };
                            }
                        }
                    });
                    for axis in 0..3 {
                        ui.horizontal(|ui| {
                            ui.label(["Width m", "Height m", "Depth m"][axis]);
                            ui.add_enabled(
                                axis != 2 || self.kind != PrimitiveKind::Plane,
                                egui::DragValue::new(&mut self.dimensions[axis]).speed(0.05),
                            );
                        });
                    }
                    if ui.button("Create editable mesh").clicked() {
                        let result = self.request(PendingAction::New(self.kind, self.dimensions));
                        self.error = result.err();
                    }
                }
                ui.separator();
                ui.small("Static triangle mesh · +Y up · metres. Export reports name data losses.");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Preview");
            if let Some(session) = &self.session {
                let picked = draw_preview(ui, session.document(), self.selection_mode, self.selected_vertex, self.selected_face);
                if let Some(picked) = picked {
                    let result = match picked {
                        Picked::Vertex(id) => self.select_vertex(id),
                        Picked::Face(id) => self.select_face(id),
                    };
                    self.error = result.err();
                }
            } else {
                ui.label("Create a cube, plane or cylinder, or import an OBJ / GLB source.");
                ui.label("The preview shows an isometric wireframe. Vertex coordinates remain authoritative in the inspector.");
            }
        });

        egui::TopBottomPanel::bottom("mesh_status").show(ctx, |ui| {
            ui.add_space(4.0);
            if let Some(error) = &self.error {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 150, 140),
                    format!("Error: {error}"),
                );
            }
            ui.label(&self.status);
            ui.add_space(4.0);
        });

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            let result = self.undo();
            self.error = result.err();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
            let result = self.redo();
            self.error = result.err();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            let result = self.save_current();
            self.error = result.err();
        }

        if self.pending.is_some() {
            let mut save = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Unsaved mesh changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Save your editable mesh before opening another document?");
                    ui.horizontal(|ui| {
                        save = ui.button("Save and continue").clicked();
                        discard = ui.button("Discard and continue").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if cancel {
                self.pending = None;
            }
            if save {
                match self.save_current() {
                    Ok(()) => {
                        if let Some(action) = self.pending.take() {
                            let result = self.perform(action);
                            self.error = result.err();
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            if discard {
                if let Some(action) = self.pending.take() {
                    let result = self.perform(action);
                    self.error = result.err();
                }
            }
        }
    }
}

fn draw_preview(
    ui: &mut egui::Ui,
    document: &Document,
    mode: SelectionMode,
    selected_vertex: Option<u64>,
    selected_face: Option<u64>,
) -> Option<Picked> {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(22, 28, 35));
    let Some((min, max)) = document.bounds() else {
        return None;
    };
    let projection =
        |p: [f64; 3]| -> [f64; 2] { [0.8660254 * (p[0] - p[2]), -p[1] - 0.5 * (p[0] + p[2])] };
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for x in [min[0], max[0]] {
        for y in [min[1], max[1]] {
            for z in [min[2], max[2]] {
                let p = projection([x, y, z]);
                for i in 0..2 {
                    lo[i] = lo[i].min(p[i]);
                    hi[i] = hi[i].max(p[i]);
                }
            }
        }
    }
    let width = (hi[0] - lo[0]).max(0.001);
    let height = (hi[1] - lo[1]).max(0.001);
    let scale = ((rect.width() - 40.0) as f64 / width)
        .min((rect.height() - 40.0) as f64 / height)
        .max(0.001);
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let screen = |p: [f64; 3]| -> egui::Pos2 {
        let q = projection(p);
        egui::pos2(
            rect.center().x + ((q[0] - center[0]) * scale) as f32,
            rect.center().y + ((q[1] - center[1]) * scale) as f32,
        )
    };
    let mut visible_triangles = 0usize;
    let mut picked = None;
    let mut nearest = 100.0f32;
    let mut face_depth = f64::NEG_INFINITY;
    let mouse = response
        .interact_pointer_pos()
        .filter(|_| response.clicked());
    for object in &document.objects {
        let edge_color = object
            .triangles
            .first()
            .and_then(|triangle| triangle.material)
            .and_then(|id| document.materials.iter().find(|material| material.id == id))
            .map(|material| {
                egui::Color32::from_rgb(
                    (material.base_color[0] * 255.0) as u8,
                    (material.base_color[1] * 255.0) as u8,
                    (material.base_color[2] * 255.0) as u8,
                )
            })
            .unwrap_or(egui::Color32::from_rgb(105, 178, 185));
        let lookup: HashMap<u64, [f64; 3]> =
            object.vertices.iter().map(|v| (v.id, v.position)).collect();
        for triangle in &object.triangles {
            if visible_triangles >= 20_000 {
                break;
            }
            let p = triangle.vertices.map(|id| screen(lookup[&id]));
            if selected_face == Some(triangle.id) {
                painter.add(egui::Shape::convex_polygon(
                    p.to_vec(),
                    egui::Color32::from_rgba_unmultiplied(255, 194, 81, 65),
                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 194, 81)),
                ));
            }
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                painter.line_segment([p[a], p[b]], egui::Stroke::new(1.0_f32, edge_color));
            }
            if mode == SelectionMode::Face {
                if let Some(mouse) = mouse {
                    if point_inside_triangle(mouse, p) {
                        let depth = triangle
                            .vertices
                            .iter()
                            .map(|id| {
                                let position = lookup[id];
                                position[0] + position[1] + position[2]
                            })
                            .sum::<f64>();
                        if depth > face_depth {
                            face_depth = depth;
                            picked = Some(Picked::Face(triangle.id));
                        }
                    }
                }
            }
            visible_triangles += 1;
        }
        if let Some(mouse) = mouse.filter(|_| mode == SelectionMode::Vertex) {
            for vertex in object.vertices.iter().take(100_000) {
                let distance = screen(vertex.position).distance_sq(mouse);
                if distance < nearest {
                    nearest = distance;
                    picked = Some(Picked::Vertex(vertex.id));
                }
            }
        }
        if let Some(vertex) = object
            .vertices
            .iter()
            .find(|v| Some(v.id) == selected_vertex)
        {
            painter.circle_filled(
                screen(vertex.position),
                5.5,
                egui::Color32::from_rgb(255, 194, 81),
            );
        }
    }
    painter.text(
        rect.left_top() + egui::vec2(12.0, 12.0),
        egui::Align2::LEFT_TOP,
        format!(
            "ISOMETRIC  ·  {} triangles{}",
            document.triangle_count(),
            if document.triangle_count() > 20_000 {
                "  ·  preview limited"
            } else {
                ""
            }
        ),
        egui::FontId::monospace(12.0),
        egui::Color32::from_rgb(166, 177, 189),
    );
    picked
}

fn point_inside_triangle(point: egui::Pos2, corners: [egui::Pos2; 3]) -> bool {
    let edge = |a: egui::Pos2, b: egui::Pos2| {
        (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x)
    };
    let a = edge(corners[0], corners[1]);
    let b = edge(corners[1], corners[2]);
    let c = edge(corners[2], corners[0]);
    let area = (corners[1].x - corners[0].x) * (corners[2].y - corners[0].y)
        - (corners[1].y - corners[0].y) * (corners[2].x - corners[0].x);
    area.abs() > 1.0 && ((a >= 0.0 && b >= 0.0 && c >= 0.0) || (a <= 0.0 && b <= 0.0 && c <= 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_panel_applies_and_undoes_object_base_color() {
        let mut panel = MeshPanel::default();
        panel
            .new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6])
            .unwrap();
        panel.material_color = [0.85, 0.28, 0.08, 1.0];
        panel.apply_material_color().unwrap();
        assert_eq!(
            panel.session.as_ref().unwrap().document().materials[0].base_color,
            [0.85, 0.28, 0.08, 1.0]
        );
        panel.undo().unwrap();
        assert!(
            panel
                .session
                .as_ref()
                .unwrap()
                .document()
                .materials
                .is_empty()
        );
    }

    #[test]
    fn native_mesh_panel_creates_edits_undoes_saves_reopens_and_exports() {
        let dir = tempfile::tempdir().unwrap();
        let document = dir.path().join("prop.mstrike");
        let glb = dir.path().join("prop.glb");
        let mut panel = MeshPanel::default();
        panel
            .new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6])
            .unwrap();
        assert!(panel.dirty());
        panel.save(&document).unwrap();
        assert!(!panel.dirty());
        let id = panel.session.as_ref().unwrap().document().objects[0].vertices[0].id;
        panel.select_vertex(id).unwrap();
        panel.edit_position[0] += 0.01;
        panel.apply_vertex_position().unwrap();
        assert!(panel.dirty());
        panel.undo().unwrap();
        assert!(!panel.dirty());
        panel.redo().unwrap();
        assert!(panel.dirty());
        panel.save(&document).unwrap();
        panel.open_document(&document).unwrap();
        assert!(!panel.dirty());
        panel.export_glb(&glb).unwrap();
        assert!(glb.is_file());
    }

    #[test]
    fn native_face_panel_selects_edits_undoes_saves_reopens_and_exports() {
        let dir = tempfile::tempdir().unwrap();
        let native = dir.path().join("face.mstrike");
        let glb = dir.path().join("face.glb");
        let mut panel = MeshPanel::default();
        panel
            .new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6])
            .unwrap();
        panel.save(&native).unwrap();
        let face = panel.session.as_ref().unwrap().document().objects[0].triangles[0].id;
        panel.select_face(face).unwrap();
        assert_eq!(panel.selection_mode, SelectionMode::Face);
        panel.inset_fraction = 0.2;
        panel.raise_metres = 0.15;
        panel.apply_face_edit().unwrap();
        assert_eq!(
            panel.session.as_ref().unwrap().document().triangle_count(),
            24
        );
        assert!(panel.dirty());
        panel.undo().unwrap();
        assert_eq!(
            panel.session.as_ref().unwrap().document().triangle_count(),
            12
        );
        assert!(!panel.dirty());
        panel.redo().unwrap();
        panel.save(&native).unwrap();
        panel.open_document(&native).unwrap();
        assert_eq!(
            panel.session.as_ref().unwrap().document().triangle_count(),
            24
        );
        panel.export_glb(&glb).unwrap();
        let bytes = std::fs::read(glb).unwrap();
        gltf::Gltf::from_slice(&bytes).unwrap();
    }
}
