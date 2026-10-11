use effectcraft_model::prim::PrimitiveKind;
use modelstrike::editor::Document;

#[test]
fn collinear_obj_face_is_rejected_by_inspector_and_editable_import() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("collinear.obj");
    std::fs::write(&source, "v 0 0 0\nv 1 0 0\nv 2 0 0\nf 1 2 3\n").unwrap();
    assert!(
        modelstrike::inspect_obj(&source)
            .unwrap_err()
            .contains("zero area")
    );
    assert!(Document::import_model(&source, 1.0).is_err());
}

#[test]
fn concave_polygon_is_rejected_before_donor_fan_can_change_its_area() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("concave.obj");
    std::fs::write(
        &source,
        "v 0 3 0\nv 0 0 0\nv 3 0 0\nv 3 1 0\nv 1 1 0\nv 1 3 0\nf 1 2 3 4 5 6\n",
    )
    .unwrap();
    assert!(
        modelstrike::inspect_obj(&source)
            .unwrap_err()
            .contains("concave")
    );
    assert!(
        Document::import_model(&source, 1.0)
            .unwrap_err()
            .contains("concave")
    );
}

#[test]
fn convex_polygon_still_imports_as_nondegenerate_triangles() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("convex.obj");
    std::fs::write(
        &source,
        "v 0 0 0\nv 2 0 0\nv 3 1 0\nv 1 3 0\nv -1 1 0\nf 1 2 3 4 5\n",
    )
    .unwrap();
    assert_eq!(
        modelstrike::inspect_obj(&source)
            .unwrap()
            .triangles_after_fan,
        3
    );
    let (document, _) = Document::import_model(&source, 1.0).unwrap();
    document.validate().unwrap();
    assert_eq!(document.triangle_count(), 3);
}

#[test]
fn nonplanar_quad_is_rejected_before_fan_import() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("nonplanar.obj");
    std::fs::write(&source, "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0.1\nf 1 2 3 4\n").unwrap();
    assert!(
        modelstrike::inspect_obj(&source)
            .unwrap_err()
            .contains("nonplanar")
    );
    assert!(Document::import_model(&source, 1.0).is_err());
}

#[test]
fn self_intersecting_star_is_rejected_before_fan_import() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("star.obj");
    std::fs::write(
        &source,
        "v 0 10 0\nv 6 -8 0\nv -9 3 0\nv 9 3 0\nv -6 -8 0\nf 1 2 3 4 5\n",
    )
    .unwrap();
    assert!(
        modelstrike::inspect_obj(&source)
            .unwrap_err()
            .contains("self-intersecting")
    );
    assert!(
        Document::import_model(&source, 1.0)
            .unwrap_err()
            .contains("self-intersecting")
    );
}

#[test]
fn inspector_and_import_reject_triangle_collapsed_by_donor_float32() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("rounded.obj");
    std::fs::write(
        &source,
        "v 100000 0 0\nv 100000.001 0 0\nv 100000 0.001 0\nf 1 2 3\n",
    )
    .unwrap();
    assert!(
        modelstrike::inspect_obj(&source)
            .unwrap_err()
            .contains("float32")
    );
    assert!(
        Document::import_model(&source, 1.0)
            .unwrap_err()
            .contains("float32")
    );
}

#[test]
fn large_coordinate_with_representable_one_metre_edges_still_imports() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("large-but-safe.obj");
    std::fs::write(
        &source,
        "v 100000 0 0\nv 100001 0 0\nv 100000 1 0\nf 1 2 3\n",
    )
    .unwrap();
    modelstrike::inspect_obj(&source).unwrap();
    Document::import_model(&source, 1.0).unwrap();
}

#[test]
fn placement_shift_over_one_percent_of_smallest_edge_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("shifted-plane.obj");
    std::fs::write(
        &source,
        "v 999999.97 0 0\nv 999999.97 1 0\nv 999999.97 0 1\nf 1 2 3\n",
    )
    .unwrap();
    let error = modelstrike::inspect_obj(&source).unwrap_err();
    assert!(error.contains("1%") && error.contains("float32"), "{error}");
    assert!(Document::import_model(&source, 1.0).is_err());
}

#[test]
fn float32_glb_export_rejects_collapsed_mesh_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("collapsed.glb");
    let mut document = Document::new_primitive(PrimitiveKind::Cube, [0.001; 3]).unwrap();
    for vertex in &mut document.objects[0].vertices {
        vertex.position[0] += 100_000.0;
    }
    document.validate().unwrap();
    let error = document.export_glb_new(&output).unwrap_err();
    assert!(error.contains("float32"), "{error}");
    assert!(!output.exists());
}

#[test]
fn small_mesh_near_origin_remains_exportable() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("small.glb");
    let document = Document::new_primitive(PrimitiveKind::Cube, [0.001; 3]).unwrap();
    document.export_glb_new(&output).unwrap();
    let bytes = std::fs::read(output).unwrap();
    let model = effectcraft_model::load("small.glb", &bytes, &|_| None).unwrap();
    assert_eq!(model.triangle_count(), 12);
}
