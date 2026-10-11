use effectcraft_model::prim::PrimitiveKind;
use modelstrike::editor::{Document, EditSession};
use std::process::Command;

#[test]
fn object_colour_is_undoable_persistent_and_in_exported_glb() {
    let dir = tempfile::tempdir().unwrap();
    let document_path = dir.path().join("colored.mstrike");
    let glb_path = dir.path().join("colored.glb");
    let source = Document::new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6]).unwrap();
    let object_id = source.objects[0].id;
    let mut session = EditSession::new(source).unwrap();
    let orange = [0.85, 0.28, 0.08, 1.0];
    session.set_object_color(object_id, orange).unwrap();
    assert_eq!(session.document().materials.len(), 1);
    assert!(
        session.document().objects[0]
            .triangles
            .iter()
            .all(|t| t.material == Some(session.document().materials[0].id))
    );
    session.undo().unwrap();
    assert!(session.document().materials.is_empty());
    session.redo().unwrap();
    session.document().save_new(&document_path).unwrap();
    let reopened = Document::load(&document_path).unwrap();
    assert_eq!(reopened.materials[0].base_color, orange);
    reopened.export_glb_new(&glb_path).unwrap();
    let gltf = gltf::Gltf::open(glb_path).unwrap();
    let color = gltf
        .materials()
        .next()
        .unwrap()
        .pbr_metallic_roughness()
        .base_color_factor();
    assert_eq!(color, orange);
}

#[test]
fn invalid_colour_does_not_change_document_or_history() {
    let source = Document::new_primitive(PrimitiveKind::Cube, [1.0, 1.0, 1.0]).unwrap();
    let mut session = EditSession::new(source.clone()).unwrap();
    assert!(
        session
            .set_object_color(source.objects[0].id, [f32::NAN, 0.0, 0.0, 1.0])
            .is_err()
    );
    assert_eq!(session.document(), &source);
    assert!(!session.can_undo());
}

#[test]
fn cli_base_color_edit_writes_new_document_and_preserves_source() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("source.mstrike");
    let colored_path = dir.path().join("colored.mstrike");
    let source = Document::new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6]).unwrap();
    let object_id = source.objects[0].id;
    source.save_new(&source_path).unwrap();
    let before = std::fs::read(&source_path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_modelstrike"))
        .arg("mesh-set-color")
        .arg(&source_path)
        .arg(&colored_path)
        .arg(object_id.to_string())
        .args(["0.85", "0.28", "0.08"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let colored = Document::load(&colored_path).unwrap();
    assert_eq!(colored.materials[0].base_color, [0.85, 0.28, 0.08, 1.0]);
    assert_eq!(std::fs::read(&source_path).unwrap(), before);
}
