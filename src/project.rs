use crate::inspect_obj;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransformProject {
    pub schema_version: u32,
    pub source_obj: String,
    pub source_sha256: String,
    pub translation: [f64; 3],
    pub scale: f64,
}

fn source_path(project: &TransformProject, document_path: &Path) -> PathBuf {
    let stored = Path::new(&project.source_obj);
    if stored.is_absolute() { stored.to_path_buf() }
    else { document_path.parent().unwrap_or_else(|| Path::new(".")).join(stored) }
}

fn validate_transform(project: &TransformProject) -> Result<(), String> {
    if !project.scale.is_finite() || project.scale <= 0.0
        || project.translation.iter().any(|value| !value.is_finite()) {
        return Err("transform project needs finite translation and positive scale".into());
    }
    Ok(())
}

pub fn new_project(source: &Path, path: &Path, translation: [f64; 3], scale: f64)
    -> Result<TransformProject, String> {
    let report = inspect_obj(source)?;
    let source = source.canonicalize().map_err(|error| error.to_string())?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."))
        .canonicalize().map_err(|error| error.to_string())?;
    let stored = source.strip_prefix(&parent).map(|relative| relative.to_path_buf())
        .unwrap_or(source);
    let project = TransformProject {
        schema_version: 1,
        source_obj: stored.to_string_lossy().into_owned(),
        source_sha256: report.source_sha256,
        translation, scale,
    };
    validate_transform(&project)?;
    Ok(project)
}

pub fn load_project(path: &Path) -> Result<(TransformProject, PathBuf), String> {
    let (project, source, warning) = load_project_recoverable(path)?;
    if let Some(warning) = warning { return Err(warning); }
    Ok((project, source))
}

/// Read saved controls even if the source moved or changed. Callers must keep
/// export disabled until the original source hash is restored or relinked.
pub fn load_project_recoverable(path: &Path) -> Result<(TransformProject, PathBuf, Option<String>), String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let project: TransformProject = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if project.schema_version != 1 { return Err("unsupported transform project version".into()); }
    validate_transform(&project)?;
    let source = source_path(&project, path);
    let warning = match inspect_obj(&source) {
        Ok(report) if report.source_sha256 == project.source_sha256 => None,
        Ok(_) => Some("source OBJ changed since transform project was saved".into()),
        Err(error) => Some(format!("source OBJ unavailable: {error}")),
    };
    Ok((project, source, warning))
}

pub fn save_project(project: &TransformProject, path: &Path, overwrite: bool) -> Result<(), String> {
    validate_transform(project)?;
    if overwrite && path.exists() {
        let (existing, _, _) = load_project_recoverable(path)?;
        if existing.source_obj != project.source_obj || existing.source_sha256 != project.source_sha256 {
            return Err("destination belongs to a different transform project".into());
        }
    } else if inspect_obj(&source_path(project, path))?.source_sha256 != project.source_sha256 {
        return Err("source OBJ changed since transform project was created".into());
    }
    let parent = path.parent().filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut pending = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    pending.write_all(&serde_json::to_vec_pretty(project).map_err(|error| error.to_string())?)
        .and_then(|_| pending.as_file().sync_all()).map_err(|error| error.to_string())?;
    if overwrite { pending.persist(path).map_err(|error| error.error.to_string())?; }
    else { pending.persist_noclobber(path).map_err(|error| error.error.to_string())?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::create_cube;

    #[test]
    fn transform_controls_reopen_and_source_change_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("cube.obj");
        let document = dir.path().join("cube.modelstrike.json");
        create_cube(2.0, &source).unwrap();
        let mut project = new_project(&source, &document, [4.0, -2.0, 1.0], 1.5).unwrap();
        save_project(&project, &document, false).unwrap();
        let (loaded, resolved) = load_project(&document).unwrap();
        assert_eq!(loaded.translation, [4.0, -2.0, 1.0]);
        assert_eq!(loaded.scale, 1.5);
        assert_eq!(resolved.canonicalize().unwrap(), source.canonicalize().unwrap());
        project.scale = 2.0;
        save_project(&project, &document, true).unwrap();
        assert_eq!(load_project(&document).unwrap().0.scale, 2.0);
        let replacement = dir.path().join("new.obj");
        create_cube(3.0, &replacement).unwrap();
        std::fs::copy(&replacement, &source).unwrap();
        assert!(load_project(&document).unwrap_err().contains("changed"));
        let (recovered, _, warning) = load_project_recoverable(&document).unwrap();
        assert_eq!(recovered.translation, [4.0, -2.0, 1.0]);
        assert_eq!(recovered.scale, 2.0);
        assert!(warning.unwrap().contains("changed"));
        std::fs::remove_file(&source).unwrap();
        let (recovered, _, warning) = load_project_recoverable(&document).unwrap();
        assert_eq!(recovered.scale, 2.0);
        assert!(warning.unwrap().contains("unavailable"));
        let mut edited = recovered;
        edited.scale = 2.5;
        save_project(&edited, &document, true).unwrap();
        assert_eq!(load_project_recoverable(&document).unwrap().0.scale, 2.5);
    }
}
