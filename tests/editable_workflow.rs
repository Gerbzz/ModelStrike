use std::process::Command;

#[test]
fn cli_creates_an_editable_meter_scale_cube() {
    let dir = tempfile::tempdir().unwrap();
    let document = dir.path().join("crate.mstrike");
    let result = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .args(["mesh-new", "cube", "1", "0.8", "0.6"])
        .arg(&document)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(document).unwrap();
    assert!(
        bytes.starts_with(b"PK"),
        "native document must be a ZIP archive"
    );
}

#[test]
fn cli_reopens_cube_with_meter_bounds_and_preserves_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let document = dir.path().join("crate.mstrike");
    let create = || {
        Command::new(env!("CARGO_BIN_EXE_modelstrike"))
            .args(["mesh-new", "cube", "1", "0.8", "0.6"])
            .arg(&document)
            .output()
            .unwrap()
    };
    assert!(create().status.success());
    let original = std::fs::read(&document).unwrap();
    let info = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-info")
        .arg(&document)
        .output()
        .unwrap();
    assert!(
        info.status.success(),
        "{}",
        String::from_utf8_lossy(&info.stderr)
    );
    let info: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(info["bounds_min"], serde_json::json!([-0.5, -0.4, -0.3]));
    assert_eq!(info["bounds_max"], serde_json::json!([0.5, 0.4, 0.3]));
    assert_eq!(info["triangles"], 12);
    assert!(
        !create().status.success(),
        "new must never clobber a document"
    );
    assert_eq!(std::fs::read(document).unwrap(), original);
}

#[test]
fn invalid_dimensions_create_no_document() {
    let dir = tempfile::tempdir().unwrap();
    let document = dir.path().join("bad.mstrike");
    let result = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .args(["mesh-new", "cube", "NaN", "1", "1"])
        .arg(&document)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!document.exists());
}

#[test]
fn vertex_edit_exports_obj_without_changing_source_document() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("crate.mstrike");
    let edited = dir.path().join("crate-edited.mstrike");
    let obj = dir.path().join("crate-edited.obj");
    let new = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .args(["mesh-new", "cube", "1", "0.8", "0.6"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(new.status.success());
    let source_bytes = std::fs::read(&source).unwrap();
    let change = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-move-vertex")
        .arg(&source)
        .arg(&edited)
        .args(["2", "2", "0", "0"])
        .output()
        .unwrap();
    assert!(
        change.status.success(),
        "{}",
        String::from_utf8_lossy(&change.stderr)
    );
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    let export = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-export-obj")
        .arg(&edited)
        .arg(&obj)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let report = modelstrike::inspect_obj(&obj).unwrap();
    assert_eq!(report.bounds_max[0], 2.0);
    assert_eq!(report.triangles_after_fan, 12);
    assert!(report.warnings.is_empty());
}

#[test]
fn rejected_vertex_edit_leaves_no_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("crate.mstrike");
    let output = dir.path().join("bad.mstrike");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_modelstrike"))
            .args(["mesh-new", "cube", "1", "1", "1"])
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let result = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-move-vertex")
        .arg(&source)
        .arg(&output)
        .args(["2", "NaN", "0", "0"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!output.exists());
}

#[test]
fn donor_obj_and_glb_fixtures_import_as_editable_documents() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/effectcraft/model/tests/fixtures");
    let dir = tempfile::tempdir().unwrap();
    for (name, expected_triangles, expected_materials) in [("cube.obj", 12, 2), ("quad.glb", 2, 1)]
    {
        let output = dir.path().join(format!("{name}.mstrike"));
        let result = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
            .arg("mesh-import")
            .arg(fixtures.join(name))
            .arg(&output)
            .arg("1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let info = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
            .arg("mesh-info")
            .arg(&output)
            .output()
            .unwrap();
        assert!(info.status.success());
        let info: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
        assert_eq!(info["triangles"], expected_triangles, "{name}");
        assert_eq!(info["materials"], expected_materials, "{name}");
    }
}

#[test]
fn exported_glb_passes_independent_parser_and_donor_reimport() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/effectcraft/model/tests/fixtures/cube.obj");
    let dir = tempfile::tempdir().unwrap();
    let document = dir.path().join("cube.mstrike");
    let output = dir.path().join("cube.glb");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_modelstrike"))
            .arg("mesh-import")
            .arg(fixture)
            .arg(&document)
            .arg("1")
            .status()
            .unwrap()
            .success()
    );
    let export = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-export-glb")
        .arg(&document)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let bytes = std::fs::read(&output).unwrap();
    let independent = gltf::Gltf::from_slice(&bytes).unwrap();
    assert_eq!(independent.materials().count(), 2);
    let reimported = effectcraft_model::load("cube.glb", &bytes, &|_| None).unwrap();
    assert_eq!(reimported.triangle_count(), 12);
    let (min, max) = reimported.bounds().unwrap();
    assert_eq!(min, [-0.5, -0.5, -0.5]);
    assert_eq!(max, [0.5, 0.5, 0.5]);
}

#[test]
fn plane_has_zero_depth_and_cylinder_rejects_non_circular_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let plane = dir.path().join("plane.mstrike");
    let result = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .args(["mesh-new", "plane", "2", "3", "0"])
        .arg(&plane)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let document = modelstrike::editor::Document::load(&plane).unwrap();
    assert_eq!(
        document.bounds().unwrap(),
        ([-1.0, -1.5, 0.0], [1.0, 1.5, 0.0])
    );
    let invalid = dir.path().join("oval-cylinder.mstrike");
    let result = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .args(["mesh-new", "cylinder", "1", "2", "3"])
        .arg(&invalid)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!invalid.exists());
}
