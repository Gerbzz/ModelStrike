use crate::inspect_obj;
use std::io::Write;
use std::path::Path;

fn new_obj_temp(output: &Path) -> Result<tempfile::NamedTempFile, String> {
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tempfile::Builder::new()
        .suffix(".obj")
        .tempfile_in(parent)
        .map_err(|error| error.to_string())
}

/// Create a cube OBJ with triangle faces, UVs, and flat face normals.
pub fn create_cube(edge: f64, output: &Path) -> Result<(), String> {
    if !edge.is_finite() || edge <= 0.0 {
        return Err("cube edge must be a positive finite number".into());
    }
    let h = edge / 2.0;
    let points = [
        [-h, -h, -h],
        [h, -h, -h],
        [h, h, -h],
        [-h, h, -h],
        [-h, -h, h],
        [h, -h, h],
        [h, h, h],
        [-h, h, h],
    ];
    let sides = [
        ([1, 4, 3, 2], [0, 0, -1]),
        ([5, 6, 7, 8], [0, 0, 1]),
        ([1, 2, 6, 5], [0, -1, 0]),
        ([4, 8, 7, 3], [0, 1, 0]),
        ([1, 5, 8, 4], [-1, 0, 0]),
        ([2, 3, 7, 6], [1, 0, 0]),
    ];
    let mut pending = new_obj_temp(output)?;
    writeln!(pending, "# ModelStrike cube edge {edge}").map_err(|error| error.to_string())?;
    for point in points {
        writeln!(pending, "v {} {} {}", point[0], point[1], point[2])
            .map_err(|error| error.to_string())?;
    }
    for uv in [[0, 0], [1, 0], [1, 1], [0, 1]] {
        writeln!(pending, "vt {} {}", uv[0], uv[1]).map_err(|error| error.to_string())?;
    }
    for (_, normal) in sides {
        writeln!(pending, "vn {} {} {}", normal[0], normal[1], normal[2])
            .map_err(|error| error.to_string())?;
    }
    for (index, (corners, _)) in sides.iter().enumerate() {
        let [a, b, c, d] = *corners;
        let normal = index + 1;
        writeln!(pending, "f {a}/1/{normal} {b}/2/{normal} {c}/3/{normal}")
            .map_err(|error| error.to_string())?;
        writeln!(pending, "f {a}/1/{normal} {c}/3/{normal} {d}/4/{normal}")
            .map_err(|error| error.to_string())?;
    }
    pending
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    inspect_obj(pending.path())?;
    pending
        .persist_noclobber(output)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

/// Translate and uniformly scale an OBJ's vertex positions. Source bytes stay
/// unchanged; faces, UVs, normals, and unrelated OBJ lines are preserved.
pub fn transform_obj(
    input: &Path,
    output: &Path,
    translation: [f64; 3],
    scale: f64,
) -> Result<(), String> {
    if !scale.is_finite() || scale <= 0.0 || translation.iter().any(|value| !value.is_finite()) {
        return Err("transform values must be finite and scale must be positive".into());
    }
    inspect_obj(input)?;
    let source = std::fs::read_to_string(input).map_err(|error| error.to_string())?;
    let mut pending = new_obj_temp(output)?;
    for (offset, line) in source.lines().enumerate() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some("v") {
            let mut position = [0f64; 3];
            for value in &mut position {
                *value = parts
                    .next()
                    .ok_or_else(|| format!("line {}: missing vertex coordinate", offset + 1))?
                    .parse::<f64>()
                    .map_err(|_| format!("line {}: invalid vertex coordinate", offset + 1))?;
            }
            let tail = parts.collect::<Vec<_>>().join(" ");
            writeln!(
                pending,
                "v {} {} {}{}{}",
                position[0] * scale + translation[0],
                position[1] * scale + translation[1],
                position[2] * scale + translation[2],
                if tail.is_empty() { "" } else { " " },
                tail,
            )
            .map_err(|error| error.to_string())?;
        } else {
            writeln!(pending, "{line}").map_err(|error| error.to_string())?;
        }
    }
    pending
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    inspect_obj(pending.path())?;
    pending
        .persist_noclobber(output)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create_cube, transform_obj};
    use crate::inspect_obj;

    #[test]
    fn creates_and_transforms_valid_obj_without_touching_source() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("cube.obj");
        create_cube(2.0, &source).unwrap();
        let original_bytes = std::fs::read(&source).unwrap();
        let original = inspect_obj(&source).unwrap();
        assert_eq!((original.vertices, original.faces), (8, 12));
        assert!(original.warnings.is_empty());
        let output = dir.path().join("moved.obj");
        transform_obj(&source, &output, [10.0, 0.0, 0.0], 2.0).unwrap();
        let changed = inspect_obj(&output).unwrap();
        assert_eq!(changed.bounds_min, [8.0, -2.0, -2.0]);
        assert_eq!(changed.bounds_max, [12.0, 2.0, 2.0]);
        assert_eq!(std::fs::read(&source).unwrap(), original_bytes);
        assert!(transform_obj(&source, &output, [0.0; 3], 1.0).is_err());
    }

    #[test]
    fn rejects_invalid_transform_before_creating_output() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("cube.obj");
        create_cube(1.0, &source).unwrap();
        let output = dir.path().join("bad.obj");
        assert!(transform_obj(&source, &output, [0.0; 3], 0.0).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn transforms_vertices_with_tabs_or_leading_spaces() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("spaced.obj");
        std::fs::write(&source, b"  v 0 0 0\nv\t1\t0\t0\nv 0 1 0\nf 1 2 3\n").unwrap();
        let output = dir.path().join("moved.obj");
        transform_obj(&source, &output, [5.0, 0.0, 0.0], 1.0).unwrap();
        let report = inspect_obj(&output).unwrap();
        assert_eq!(report.bounds_min[0], 5.0);
        assert_eq!(report.bounds_max[0], 6.0);
    }
}
