use effectcraft_model::prim::PrimitiveKind;
use modelstrike::editor::{Document, EditSession};
use std::process::Command;

fn cube() -> Document {
    Document::new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6]).unwrap()
}

#[test]
fn face_inset_raise_is_one_reversible_edit_with_stable_cap_id() {
    let original = cube();
    let face = original.objects[0].triangles[0].id;
    let other_ids: Vec<u64> = original.objects[0].triangles[1..]
        .iter()
        .map(|triangle| triangle.id)
        .collect();
    let mut session = EditSession::new(original.clone()).unwrap();
    session.inset_raise_triangle(face, 0.2, 0.15).unwrap();
    let edited = session.document().clone();
    edited.validate().unwrap();
    assert_eq!(
        edited.objects[0].vertices.len(),
        original.objects[0].vertices.len() + 6
    );
    assert_eq!(edited.triangle_count(), original.triangle_count() + 12);
    assert!(edited.objects[0].triangles.iter().any(|t| t.id == face));
    assert!(
        other_ids
            .iter()
            .all(|id| edited.objects[0].triangles.iter().any(|t| t.id == *id))
    );
    session.undo().unwrap();
    assert_eq!(*session.document(), original);
    assert!(!session.can_undo());
    session.redo().unwrap();
    assert_eq!(*session.document(), edited);
}

#[test]
fn invalid_face_edit_preserves_document_and_history() {
    let original = cube();
    let face = original.objects[0].triangles[0].id;
    let mut session = EditSession::new(original.clone()).unwrap();
    for (id, inset, raise) in [
        (u64::MAX, 0.2, 0.1),
        (face, 0.0, 0.1),
        (face, 0.5, 0.1),
        (face, f64::NAN, 0.1),
        (face, 0.2, 0.0),
        (face, 0.2, -0.1),
        (face, 0.2, f64::INFINITY),
    ] {
        assert!(session.inset_raise_triangle(id, inset, raise).is_err());
        assert_eq!(*session.document(), original);
        assert!(!session.can_undo());
        assert!(!session.can_redo());
    }
}

#[test]
fn face_edit_round_trips_and_exports_valid_glb() {
    let dir = tempfile::tempdir().unwrap();
    let native = dir.path().join("raised.mstrike");
    let glb = dir.path().join("raised.glb");
    let obj = dir.path().join("raised.obj");
    let mut edited = cube();
    let object_id = edited.objects[0].id;
    edited
        .set_object_color(object_id, [0.85, 0.28, 0.08, 1.0])
        .unwrap();
    let face = edited.objects[0].triangles[0].id;
    let material = edited.objects[0].triangles[0].material;
    edited.inset_raise_triangle(face, 0.2, 0.15).unwrap();
    assert!(
        edited.objects[0]
            .triangles
            .iter()
            .all(|t| t.material == material)
    );
    edited.save_new(&native).unwrap();
    let reopened = Document::load(&native).unwrap();
    assert_eq!(reopened, edited);
    reopened.export_glb_new(&glb).unwrap();
    reopened.export_obj_new(&obj).unwrap();
    let obj_text = std::fs::read_to_string(&obj).unwrap();
    assert_eq!(
        obj_text
            .lines()
            .filter(|line| line.starts_with("vn "))
            .count(),
        24
    );
    assert_eq!(
        modelstrike::inspect_obj(&obj).unwrap().triangles_after_fan,
        24
    );
    let bytes = std::fs::read(&glb).unwrap();
    let independent = gltf::Gltf::from_slice(&bytes).unwrap();
    let color = independent
        .materials()
        .next()
        .unwrap()
        .pbr_metallic_roughness()
        .base_color_factor();
    assert_eq!(color, [0.85, 0.28, 0.08, 1.0]);
    let reimported = effectcraft_model::load("raised.glb", &bytes, &|_| None).unwrap();
    assert_eq!(reimported.triangle_count(), edited.triangle_count());
    let (export_min, export_max) = reimported.bounds().unwrap();
    let (edit_min, edit_max) = edited.bounds().unwrap();
    for axis in 0..3 {
        assert!((export_min[axis] - edit_min[axis]).abs() < 0.0001);
        assert!((export_max[axis] - edit_max[axis]).abs() < 0.0001);
    }
}

#[test]
fn invalid_source_and_id_overflow_cannot_partially_edit() {
    let mut invalid = cube();
    let face = invalid.objects[0].triangles[0].id;
    invalid.objects[0].triangles[0].vertices[0] = u64::MAX;
    let before = invalid.clone();
    assert!(invalid.inset_raise_triangle(face, 0.2, 0.1).is_err());
    assert_eq!(invalid, before);

    let mut overflow = cube();
    overflow.next_id = u64::MAX - 17;
    let before = overflow.clone();
    assert!(overflow.inset_raise_triangle(face, 0.2, 0.1).is_err());
    assert_eq!(overflow, before);
}

#[test]
fn cli_face_edit_produces_new_document_and_rejects_bad_input_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mstrike");
    let edited = dir.path().join("edited.mstrike");
    let rejected = dir.path().join("rejected.mstrike");
    cube().save_new(&source).unwrap();
    let original = std::fs::read(&source).unwrap();
    let face = cube().objects[0].triangles[0].id.to_string();
    let ok = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-inset-raise-face")
        .arg(&source)
        .arg(&edited)
        .args([&face, "0.2", "0.15"])
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert_eq!(Document::load(&edited).unwrap().triangle_count(), 24);
    let bad = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-inset-raise-face")
        .arg(&source)
        .arg(&rejected)
        .args([&face, "0.2", "-1"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(!rejected.exists());
    assert_eq!(std::fs::read(&source).unwrap(), original);
}
