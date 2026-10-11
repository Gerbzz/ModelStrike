use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

pub mod editor;
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

fn donor_number(token: Option<&str>, line: usize) -> Result<f64, String> {
    let value = token
        .ok_or_else(|| format!("line {line}: missing number"))?
        .parse::<f32>()
        .map_err(|_| format!("line {line}: coordinate cannot be represented as float32"))?;
    if !value.is_finite() {
        return Err(format!("line {line}: coordinate exceeds float32 range"));
    }
    Ok(f64::from(value))
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

fn segments_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], span: f64) -> bool {
    let orientation = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let area_epsilon = 1e-12 * span * span;
    let position_epsilon = 1e-12 * span;
    let side = |value: f64| {
        if value > area_epsilon {
            1
        } else if value < -area_epsilon {
            -1
        } else {
            0
        }
    };
    let on_segment = |point: [f64; 2], first: [f64; 2], second: [f64; 2]| {
        (0..2).all(|axis| {
            point[axis] >= first[axis].min(second[axis]) - position_epsilon
                && point[axis] <= first[axis].max(second[axis]) + position_epsilon
        })
    };
    let ab_c = side(orientation(a, b, c));
    let ab_d = side(orientation(a, b, d));
    let cd_a = side(orientation(c, d, a));
    let cd_b = side(orientation(c, d, b));
    (ab_c * ab_d < 0 && cd_a * cd_b < 0)
        || (ab_c == 0 && on_segment(c, a, b))
        || (ab_d == 0 && on_segment(d, a, b))
        || (cd_a == 0 && on_segment(a, c, d))
        || (cd_b == 0 && on_segment(b, c, d))
}

fn validate_simple_face(points: &[[f64; 3]], line: usize, precision: &str) -> Result<(), String> {
    let subtract =
        |a: [f64; 3], b: [f64; 3]| -> [f64; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] };
    let cross = |a: [f64; 3], b: [f64; 3]| -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };
    let normal = cross(
        subtract(points[1], points[0]),
        subtract(points[2], points[0]),
    );
    let length = dot(normal, normal).sqrt();
    if length < 1e-10 {
        return Err(format!("line {line}: face has zero area{precision}"));
    }
    let unit = normal.map(|value| value / length);
    let mut min = points[0];
    let mut max = points[0];
    for point in points {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    let span = dot(subtract(max, min), subtract(max, min)).sqrt().max(1.0);
    for point in &points[3..] {
        if dot(subtract(*point, points[0]), unit).abs() > 1e-8 * span {
            return Err(format!(
                "line {line}: nonplanar OBJ polygon is unsupported{precision}"
            ));
        }
    }
    let dropped_axis = (0..3)
        .max_by(|&first, &second| unit[first].abs().total_cmp(&unit[second].abs()))
        .unwrap();
    let axes = match dropped_axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    let projected: Vec<[f64; 2]> = points
        .iter()
        .map(|point| [point[axes[0]], point[axes[1]]])
        .collect();
    for first in 0..points.len() {
        for second in first + 1..points.len() {
            if second == first + 1 || (first == 0 && second == points.len() - 1) {
                continue;
            }
            if segments_intersect(
                projected[first],
                projected[(first + 1) % points.len()],
                projected[second],
                projected[(second + 1) % points.len()],
                span,
            ) {
                return Err(format!(
                    "line {line}: self-intersecting OBJ polygon is unsupported{precision}"
                ));
            }
        }
    }
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        let third = points[(index + 2) % points.len()];
        let turn = dot(
            cross(subtract(second, first), subtract(third, second)),
            unit,
        );
        if turn < -1e-10 * span * span {
            return Err(format!(
                "line {line}: concave OBJ polygon is unsupported{precision}"
            ));
        }
    }
    for index in 2..points.len() {
        let triangle = cross(
            subtract(points[index - 1], points[0]),
            subtract(points[index], points[0]),
        );
        if dot(triangle, triangle).sqrt() < 1e-10 {
            return Err(format!(
                "line {line}: fan triangle has zero area{precision}"
            ));
        }
    }
    Ok(())
}

fn validate_face_geometry(
    positions: &[[f64; 3]],
    represented_positions: &[[f64; 3]],
    corners: &[usize],
    line: usize,
) -> Result<(), String> {
    let original: Vec<[f64; 3]> = corners.iter().map(|&index| positions[index]).collect();
    let represented: Vec<[f64; 3]> = corners
        .iter()
        .map(|&index| represented_positions[index])
        .collect();
    validate_simple_face(&original, line, "")?;
    validate_simple_face(&represented, line, " at float32 OBJ precision")?;
    for index in 2..original.len() {
        let corner_ids = [0, index - 1, index];
        let triangle = corner_ids.map(|corner| original[corner]);
        let rounded = corner_ids.map(|corner| represented[corner]);
        let cross_length = |points: [[f64; 3]; 3]| {
            let a = [
                points[1][0] - points[0][0],
                points[1][1] - points[0][1],
                points[1][2] - points[0][2],
            ];
            let b = [
                points[2][0] - points[0][0],
                points[2][1] - points[0][1],
                points[2][2] - points[0][2],
            ];
            let cross = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            cross
                .iter()
                .map(|component| component * component)
                .sum::<f64>()
                .sqrt()
        };
        let original_area = cross_length(triangle);
        let rounded_area = cross_length(rounded);
        let shortest_edge = [(0, 1), (1, 2), (2, 0)]
            .map(|(a, b)| {
                (0..3)
                    .map(|axis| (triangle[a][axis] - triangle[b][axis]).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .into_iter()
            .fold(f64::INFINITY, f64::min);
        let max_position_error = (0..3)
            .map(|corner| {
                (0..3)
                    .map(|axis| (triangle[corner][axis] - rounded[corner][axis]).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .fold(0.0, f64::max);
        if !(0.99..=1.01).contains(&(rounded_area / original_area))
            || max_position_error > shortest_edge * 0.01
        {
            return Err(format!(
                "line {line}: OBJ face loses shape beyond 1% at float32 donor precision"
            ));
        }
    }
    Ok(())
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
    inspect_obj_bytes(&bytes)
}

/// Validate the exact bytes that the donor OBJ parser will receive.
pub(crate) fn inspect_obj_bytes(bytes: &[u8]) -> Result<PropReport, String> {
    let source = std::str::from_utf8(bytes).map_err(|_| "OBJ must be UTF-8 text".to_string())?;
    let mut vertices = 0usize;
    let mut texcoords = 0usize;
    let mut normals = 0usize;
    let mut faces = 0usize;
    let mut triangles = 0usize;
    let mut faces_without_uv = 0usize;
    let mut faces_without_normal = 0usize;
    let mut positions = Vec::new();
    let mut represented_positions = Vec::new();
    let mut bounds_min = [f64::INFINITY; 3];
    let mut bounds_max = [f64::NEG_INFINITY; 3];
    for (offset, raw) in source.lines().enumerate() {
        let line = offset + 1;
        let content = raw.split('#').next().unwrap_or("").trim();
        let mut parts = content.split_whitespace();
        match parts.next() {
            Some("v") => {
                let tokens = [parts.next(), parts.next(), parts.next()];
                let coordinates = [
                    number(tokens[0], line)?,
                    number(tokens[1], line)?,
                    number(tokens[2], line)?,
                ];
                let represented = [
                    donor_number(tokens[0], line)?,
                    donor_number(tokens[1], line)?,
                    donor_number(tokens[2], line)?,
                ];
                positions.push(coordinates);
                represented_positions.push(represented);
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
                if corners.len() > 1024 {
                    return Err(format!(
                        "line {line}: face exceeds 1024-corner validation limit"
                    ));
                }
                let mut distinct = HashSet::new();
                let mut missing_uv = false;
                let mut missing_normal = false;
                let mut face_vertices = Vec::with_capacity(corners.len());
                for corner in &corners {
                    let fields: Vec<_> = corner.split('/').collect();
                    if fields.len() > 3 {
                        return Err(format!("line {line}: invalid face corner"));
                    }
                    let vertex = obj_index(fields[0], vertices, line)?;
                    if !distinct.insert(vertex) {
                        return Err(format!("line {line}: repeated vertex in face"));
                    }
                    face_vertices.push(vertex);
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
                validate_face_geometry(&positions, &represented_positions, &face_vertices, line)?;
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
    hash.update(bytes);
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
