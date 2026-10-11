use crate::mesh::{create_cube, transform_obj};
use crate::project::{TransformProject, load_project_recoverable, new_project, save_project};
use crate::{PropReport, inspect_obj, write_new_report};
use eframe::egui;
use std::path::{Path, PathBuf};

mod mesh;

pub struct ModelStrikeApp {
    mesh_panel: mesh::MeshPanel,
    mesh_mode: bool,
    pub source: Option<PathBuf>,
    pub report: Option<PropReport>,
    pub cube_edge: f64,
    pub cube_path: String,
    pub transform_path: String,
    pub report_path: String,
    pub project_path: String,
    pub translation: [f64; 3],
    pub scale: f64,
    pub status: Option<String>,
    pub error: Option<String>,
    pub source_warning: Option<String>,
    expected_source_sha256: Option<String>,
    loaded_project: Option<TransformProject>,
    loaded_project_path: Option<PathBuf>,
    saved_controls: ([f64; 3], f64),
    pending_open: Option<PendingOpen>,
}

enum PendingOpen {
    Obj(PathBuf),
    Project(PathBuf),
    Cube,
}

impl Default for ModelStrikeApp {
    fn default() -> Self {
        Self {
            mesh_panel: mesh::MeshPanel::default(),
            mesh_mode: true,
            source: None,
            report: None,
            cube_edge: 64.0,
            cube_path: String::new(),
            transform_path: String::new(),
            report_path: String::new(),
            project_path: String::new(),
            translation: [0.0; 3],
            scale: 1.0,
            status: None,
            error: None,
            source_warning: None,
            expected_source_sha256: None,
            loaded_project: None,
            loaded_project_path: None,
            saved_controls: ([0.0; 3], 1.0),
            pending_open: None,
        }
    }
}

impl ModelStrikeApp {
    pub fn open_mesh_document(&mut self, path: &Path) -> Result<(), String> {
        self.mesh_panel.open_document(path)
    }

    pub fn select_mesh_vertex(&mut self, id: u64) -> Result<(), String> {
        self.mesh_panel.select_vertex(id)
    }

    pub fn select_mesh_face(&mut self, id: u64) -> Result<(), String> {
        self.mesh_panel.select_face(id)
    }

    fn has_unsaved_controls(&self) -> bool {
        self.source.is_some() && (self.translation, self.scale) != self.saved_controls
    }

    pub fn open_obj(&mut self, path: &Path) -> Result<(), String> {
        if self.has_unsaved_controls() {
            self.pending_open = Some(PendingOpen::Obj(path.to_path_buf()));
            return Ok(());
        }
        self.open_obj_unchecked(path)
    }

    fn open_obj_unchecked(&mut self, path: &Path) -> Result<(), String> {
        match inspect_obj(path) {
            Ok(report) => {
                self.expected_source_sha256 = Some(report.source_sha256.clone());
                self.loaded_project = None;
                self.loaded_project_path = None;
                self.source = Some(path.to_path_buf());
                self.transform_path = path
                    .with_file_name(format!(
                        "{}-transformed.obj",
                        path.file_stem().unwrap_or_default().to_string_lossy()
                    ))
                    .display()
                    .to_string();
                self.report_path = path
                    .with_extension("modelstrike.json")
                    .display()
                    .to_string();
                self.project_path = path
                    .with_extension("modelstrike-project.json")
                    .display()
                    .to_string();
                self.report = Some(report);
                self.translation = [0.0; 3];
                self.scale = 1.0;
                self.saved_controls = (self.translation, self.scale);
                self.source_warning = None;
                self.status = Some("OBJ geometry loaded".into());
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn create_primitive(&mut self) -> Result<(), String> {
        if self.has_unsaved_controls() {
            self.pending_open = Some(PendingOpen::Cube);
            return Ok(());
        }
        self.create_primitive_unchecked()
    }

    fn create_primitive_unchecked(&mut self) -> Result<(), String> {
        match create_cube(self.cube_edge, Path::new(&self.cube_path)) {
            Ok(()) => self.open_obj_unchecked(&Path::new(&self.cube_path).to_path_buf()),
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn open_transform_project(&mut self, path: &Path) -> Result<(), String> {
        if self.has_unsaved_controls() {
            self.pending_open = Some(PendingOpen::Project(path.to_path_buf()));
            return Ok(());
        }
        self.open_transform_project_unchecked(path)
    }

    fn open_transform_project_unchecked(&mut self, path: &Path) -> Result<(), String> {
        match load_project_recoverable(path) {
            Ok((project, source, warning)) => {
                self.report = inspect_obj(&source).ok();
                self.source = Some(source.clone());
                self.expected_source_sha256 = Some(project.source_sha256.clone());
                self.loaded_project = Some(project.clone());
                self.loaded_project_path = Some(path.to_path_buf());
                self.source_warning = warning;
                self.translation = project.translation;
                self.scale = project.scale;
                self.saved_controls = (self.translation, self.scale);
                self.project_path = path.display().to_string();
                self.transform_path = source
                    .with_file_name(format!(
                        "{}-transformed.obj",
                        source.file_stem().unwrap_or_default().to_string_lossy()
                    ))
                    .display()
                    .to_string();
                self.report_path = source
                    .with_extension("modelstrike.json")
                    .display()
                    .to_string();
                self.status = Some(if self.source_warning.is_some() {
                    "Transform controls recovered; source needs repair before export".into()
                } else {
                    "Transform project reopened".into()
                });
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn discard_pending_open(&mut self) -> Result<(), String> {
        match self.pending_open.take() {
            Some(PendingOpen::Obj(path)) => self.open_obj_unchecked(&path),
            Some(PendingOpen::Project(path)) => self.open_transform_project_unchecked(&path),
            Some(PendingOpen::Cube) => self.create_primitive_unchecked(),
            None => Ok(()),
        }
    }

    fn check_expected_source(&self) -> Result<(), String> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| "create or open an OBJ first".to_string())?;
        let actual = inspect_obj(source)?.source_sha256;
        if self.expected_source_sha256.as_ref() != Some(&actual) {
            return Err(
                "source OBJ changed; saved transform controls are preserved but export is blocked"
                    .into(),
            );
        }
        Ok(())
    }

    pub fn save_transform_project(&mut self) -> Result<(), String> {
        let path = PathBuf::from(&self.project_path);
        let result = (|| {
            let project = if self.loaded_project_path.as_deref() == Some(path.as_path()) {
                let mut project = self
                    .loaded_project
                    .clone()
                    .ok_or("transform project is unavailable")?;
                project.translation = self.translation;
                project.scale = self.scale;
                project
            } else {
                self.check_expected_source()?;
                let source = self.source.as_ref().ok_or("create or open an OBJ first")?;
                new_project(source, &path, self.translation, self.scale)?
            };
            save_project(&project, &path, path.exists())?;
            Ok::<TransformProject, String>(project)
        })();
        match result {
            Ok(project) => {
                self.loaded_project = Some(project);
                self.loaded_project_path = Some(path);
                self.status = Some(format!("Transform project saved: {}", self.project_path));
                self.saved_controls = (self.translation, self.scale);
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn transform(&mut self) -> Result<(), String> {
        if let Err(error) = self.check_expected_source() {
            self.error = Some(error.clone());
            return Err(error);
        }
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| "create or open an OBJ first".to_string())?;
        match transform_obj(
            source,
            Path::new(&self.transform_path),
            self.translation,
            self.scale,
        ) {
            Ok(()) => {
                self.status = Some(format!("Transformed OBJ saved: {}", self.transform_path));
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn save_inspection(&mut self) -> Result<(), String> {
        let report = self
            .report
            .as_ref()
            .ok_or_else(|| "create or open an OBJ first".to_string())?;
        match write_new_report(report, Path::new(&self.report_path)) {
            Ok(()) => {
                self.status = Some(format!("Inspection report saved: {}", self.report_path));
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }
}

impl eframe::App for ModelStrikeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 7.0);
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(29, 36, 42);
            visuals.window_fill = egui::Color32::from_rgb(36, 43, 49);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 55, 62);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 75, 81);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(43, 107, 117);
            visuals.selection.bg_fill = egui::Color32::from_rgb(37, 118, 130);
            style.visuals = visuals;
        });
        egui::TopBottomPanel::top("modelstrike_navigation").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("MODELSTRIKE");
                ui.separator();
                ui.selectable_value(&mut self.mesh_mode, true, "Mesh editor");
                ui.selectable_value(&mut self.mesh_mode, false, "OBJ utility");
                ui.label("Local prop authoring");
            });
        });
        if self.mesh_mode {
            self.mesh_panel.show(ctx);
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ModelStrike");
            ui.label("Custom-map prop source: create a cube or transform an OBJ.");
            ui.horizontal(|ui| {
                if ui.button("Open OBJ").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("OBJ model", &["obj"])
                        .pick_file()
                    {
                        let _ = self.open_obj(&path);
                    }
                }
                if ui.button("Open transform project").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("ModelStrike project", &["json"])
                        .pick_file()
                    {
                        let _ = self.open_transform_project(&path);
                    }
                }
                ui.label("Cube edge");
                ui.add(egui::DragValue::new(&mut self.cube_edge).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("New cube OBJ");
                ui.text_edit_singleline(&mut self.cube_path);
            });
            if ui.button("Create cube").clicked() {
                let _ = self.create_primitive();
            }
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if let Some(status) = &self.status {
                ui.colored_label(egui::Color32::LIGHT_GREEN, status);
            }
            if let Some(warning) = &self.source_warning {
                ui.colored_label(egui::Color32::YELLOW, warning);
            }
            if let Some(report) = &self.report {
                ui.separator();
                if let Some(source) = &self.source {
                    ui.small(source.display().to_string());
                }
                ui.label(format!(
                    "{} vertices · {} faces · {} triangles",
                    report.vertices, report.faces, report.triangles_after_fan
                ));
                ui.label(format!(
                    "Bounds {:?} to {:?}",
                    report.bounds_min, report.bounds_max
                ));
                for warning in &report.warnings {
                    ui.colored_label(egui::Color32::YELLOW, warning);
                }
            }
            if self.source.is_some() {
                if self.report.is_none() {
                    ui.label("Saved controls remain available while the source OBJ is missing.");
                }
                ui.horizontal(|ui| {
                    ui.label("Translate XYZ");
                    for value in &mut self.translation {
                        ui.add(egui::DragValue::new(value));
                    }
                    ui.label("Scale");
                    ui.add(egui::DragValue::new(&mut self.scale).speed(0.1));
                });
                ui.horizontal(|ui| {
                    ui.label("New transformed OBJ");
                    ui.text_edit_singleline(&mut self.transform_path);
                });
                if ui.button("Save transformed OBJ").clicked() {
                    let _ = self.transform();
                }
                ui.horizontal(|ui| {
                    ui.label("Transform project");
                    ui.text_edit_singleline(&mut self.project_path);
                });
                if ui.button("Save transform project").clicked() {
                    let _ = self.save_transform_project();
                }
                ui.horizontal(|ui| {
                    ui.label("Inspection report");
                    ui.text_edit_singleline(&mut self.report_path);
                });
                if ui.button("Save report").clicked() {
                    let _ = self.save_inspection();
                }
                ui.small("OBJ output is not a compiled Source 2 model.");
            }
        });
        if self.pending_open.is_some() {
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Unsaved transform controls")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(
                        "Save the current transform project or discard these control changes.",
                    );
                    ui.horizontal(|ui| {
                        discard = ui.button("Discard changes and continue").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if cancel {
                self.pending_open = None;
            }
            if discard {
                if let Err(error) = self.discard_pending_open() {
                    self.error = Some(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ModelStrikeApp;
    use crate::inspect_obj;
    use std::path::Path;

    #[test]
    fn view_model_creates_transforms_and_reopens_obj() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut app = ModelStrikeApp::default();
        app.cube_path = dir.path().join("cube.obj").display().to_string();
        app.cube_edge = 2.0;
        app.create_primitive().unwrap();
        assert_eq!(app.report.as_ref().unwrap().faces, 12);
        app.translation = [10.0, 0.0, 0.0];
        app.scale = 1.25;
        app.save_transform_project().unwrap();
        let project_path = app.project_path.clone();
        app.translation = [0.0; 3];
        app.scale = 1.0;
        app.open_transform_project(Path::new(&project_path))
            .unwrap();
        assert_eq!(app.translation, [0.0; 3]);
        app.discard_pending_open().unwrap();
        assert_eq!(app.translation, [10.0, 0.0, 0.0]);
        assert_eq!(app.scale, 1.25);
        app.transform().unwrap();
        let changed = inspect_obj(Path::new(&app.transform_path)).unwrap();
        assert_eq!(changed.bounds_min[0], 8.75);
        app.open_obj(&Path::new(&app.transform_path).to_path_buf())
            .unwrap();
        assert_eq!(app.report.as_ref().unwrap().bounds_min[0], 8.75);
    }

    #[test]
    fn changed_source_recovers_saved_controls_and_blocks_export() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("source.obj");
        let replacement = dir.path().join("replacement.obj");
        crate::mesh::create_cube(2.0, &source).unwrap();
        crate::mesh::create_cube(3.0, &replacement).unwrap();
        let mut app = ModelStrikeApp::default();
        app.open_obj(&source).unwrap();
        app.translation = [7.0, 2.0, -1.0];
        app.scale = 1.5;
        app.save_transform_project().unwrap();
        let project_path = app.project_path.clone();
        std::fs::copy(&replacement, &source).unwrap();
        let mut reopened = ModelStrikeApp::default();
        reopened
            .open_transform_project(Path::new(&project_path))
            .unwrap();
        assert_eq!(reopened.translation, [7.0, 2.0, -1.0]);
        assert_eq!(reopened.scale, 1.5);
        assert!(
            reopened
                .source_warning
                .as_ref()
                .unwrap()
                .contains("changed")
        );
        assert!(reopened.transform().unwrap_err().contains("changed"));
        reopened.scale = 2.0;
        reopened.save_transform_project().unwrap();
        let mut saved_again = ModelStrikeApp::default();
        saved_again
            .open_transform_project(Path::new(&project_path))
            .unwrap();
        assert_eq!(saved_again.scale, 2.0);
        assert!(
            saved_again
                .source_warning
                .as_ref()
                .unwrap()
                .contains("changed")
        );
        assert!(saved_again.transform().is_err());
    }
}
