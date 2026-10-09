use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

pub mod mesh;
pub mod project;

#[cfg(feature = "native-ui")]
pub mod ui;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropReport {
    pub schema_version: u32,
    pub source_sha256: String,
    pub vertices: usize,
    pub texture_coordinates: usize,
    pub normals: usize,
    pub faces: usize,
    pub triangles_after_fan: usize,
    pub bounds_min: [f64; 3],
    pub bounds_max: [f64; 3],
    pub warnings: Vec<String>,
}

fn number(token: Option<&str>, line: usize) -> Result<f64, String> {
    let value = token
        .ok_or_else(|| format!("line {line}: missing number"))?
        .parse::<f64>()
        .map_err(|_| format!("line {line}: invalid number"))?;
    if !value.is_finite() {
        return Err(format!("line {line}: non-finite coordinate"));
    }
    Ok(value)
}

fn obj_index(token: &str, count: usize, line: usize) -> Result<usize, String> {
    let value = token
        .parse::<i64>()
        .map_err(|_| format!("line {line}: invalid face index"))?;
    let resolved = if value > 0 {
        value - 1
    } else {
        count as i64 + value
    };
    if value == 0 || resolved < 0 || resolved >= count as i64 {
        return Err(format!("line {line}: face index outside declared geometry"));
    }
    Ok(resolved as usize)
}

/// Validate OBJ geometry for a map prop. This does not bake materials or
/// produce a Source 2 model; warnings identify missing UVs and normals.
pub fn inspect_obj(path: &Path) -> Result<PropReport, String> {
    if !path.is_file()
        || !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("obj"))
    {
        return Err("select an existing OBJ prop source".into());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let source = std::str::from_utf8(&bytes).map_err(|_| "OBJ must be UTF-8 text".to_string())?;
    let mut vertices = 0usize;
    let mut texcoords = 0usize;
    let mut normals = 0usize;
    let mut faces = 0usize;
    let mut triangles = 0usize;
    let mut faces_without_uv = 0usize;
    let mut faces_without_normal = 0usize;
    let mut bounds_min = [f64::INFINITY; 3];
    let mut bounds_max = [f64::NEG_INFINITY; 3];
    for (offset, raw) in source.lines().enumerate() {
        let line = offset + 1;
        let content = raw.split('#').next().unwrap_or("").trim();
        let mut parts = content.split_whitespace();
        match parts.next() {
            Some("v") => {
                let coordinates = [
                    number(parts.next(), line)?,
                    number(parts.next(), line)?,
                    number(parts.next(), line)?,
                ];
                for axis in 0..3 {
                    bounds_min[axis] = bounds_min[axis].min(coordinates[axis]);
                    bounds_max[axis] = bounds_max[axis].max(coordinates[axis]);
                }
                vertices += 1;
            }
            Some("vt") => {
                number(parts.next(), line)?;
                number(parts.next(), line)?;
                texcoords += 1;
            }
            Some("vn") => {
                let vector = [
                    number(parts.next(), line)?,
                    number(parts.next(), line)?,
                    number(parts.next(), line)?,
                ];
                if vector == [0.0, 0.0, 0.0] {
                    return Err(format!("line {line}: zero normal"));
                }
                normals += 1;
            }
            Some("f") => {
                let corners: Vec<_> = parts.collect();
                if corners.len() < 3 {
                    return Err(format!("line {line}: face has fewer than three corners"));
                }
                let mut distinct = HashSet::new();
                let mut missing_uv = false;
                let mut missing_normal = false;
                for corner in &corners {
                    let fields: Vec<_> = corner.split('/').collect();
                    if fields.len() > 3 {
                        return Err(format!("line {line}: invalid face corner"));
                    }
                    let vertex = obj_index(fields[0], vertices, line)?;
                    if !distinct.insert(vertex) {
                        return Err(format!("line {line}: repeated vertex in face"));
                    }
                    if fields.get(1).is_some_and(|field| !field.is_empty()) {
                        obj_index(fields[1], texcoords, line)?;
                    } else {
                        missing_uv = true;
                    }
                    if fields.get(2).is_some_and(|field| !field.is_empty()) {
                        obj_index(fields[2], normals, line)?;
                    } else {
                        missing_normal = true;
                    }
                }
                faces += 1;
                triangles += corners.len() - 2;
                if missing_uv {
                    faces_without_uv += 1;
                }
                if missing_normal {
                    faces_without_normal += 1;
                }
            }
            _ => {}
        }
    }
    if vertices == 0 || faces == 0 {
        return Err("OBJ needs vertices and faces".into());
    }
    let mut warnings = Vec::new();
    if faces_without_uv > 0 {
        warnings.push(format!(
            "{faces_without_uv} faces lack complete UV coordinates"
        ));
    }
    if faces_without_normal > 0 {
        warnings.push(format!(
            "{faces_without_normal} faces lack complete normals"
        ));
    }
    let mut hash = Sha256::new();
    hash.update(&bytes);
    Ok(PropReport {
        schema_version: 1,
        source_sha256: format!("{:x}", hash.finalize()),
        vertices,
        texture_coordinates: texcoords,
        normals,
        faces,
        triangles_after_fan: triangles,
        bounds_min,
        bounds_max,
        warnings,
    })
}

pub fn write_new_report(report: &PropReport, output: &Path) -> Result<(), String> {
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut pending = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
    pending
        .write_all(&bytes)
        .and_then(|_| pending.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    pending
        .persist_noclobber(output)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PropReport, inspect_obj, write_new_report};
    use std::fs;

    #[test]
    fn validates_quad_with_uv_normals_and_reopens_report() {
        let dir = tempfile::TempDir::new().unwrap();
        let input = dir.path().join("prop.obj");
        fs::write(&input, b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1 4/4/1\n").unwrap();
        let report = inspect_obj(&input).unwrap();
        assert_eq!(
            (report.vertices, report.faces, report.triangles_after_fan),
            (4, 1, 2)
        );
        assert_eq!(report.bounds_min, [0.0, 0.0, 0.0]);
        assert_eq!(report.bounds_max, [1.0, 1.0, 0.0]);
        assert!(report.warnings.is_empty());
        let output = dir.path().join("prop-report.json");
        write_new_report(&report, &output).unwrap();
        let reopened: PropReport = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(reopened, report);
        assert!(write_new_report(&report, &output).is_err());
    }

    #[test]
    fn handles_negative_indices_and_reports_missing_uv_normals() {
        let dir = tempfile::TempDir::new().unwrap();
        let input = dir.path().join("prop.obj");
        fs::write(&input, b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n").unwrap();
        let report = inspect_obj(&input).unwrap();
        assert_eq!(report.triangles_after_fan, 1);
        assert_eq!(report.warnings.len(), 2);
    }

    #[test]
    fn rejects_out_of_range_face_index() {
        let dir = tempfile::TempDir::new().unwrap();
        let input = dir.path().join("bad.obj");
        fs::write(&input, b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n").unwrap();
        assert!(
            inspect_obj(&input)
                .unwrap_err()
                .contains("outside declared geometry")
        );
    }
}
