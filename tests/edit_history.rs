use effectcraft_model::prim::PrimitiveKind;
use modelstrike::editor::{Document, EditSession};

#[test]
fn twenty_vertex_edits_undo_and_redo_without_losing_stable_ids() {
    let original = Document::new_primitive(PrimitiveKind::Cube, [1.0, 0.8, 0.6]).unwrap();
    let id = original.objects[0].vertices[0].id;
    let initial = original.objects[0].vertices[0].position;
    let mut editor = EditSession::new(original.clone()).unwrap();
    for step in 1..=20 {
        let mut position = initial;
        position[0] += step as f64 * 0.001;
        editor.move_vertex(id, position).unwrap();
    }
    let edited = editor.document().clone();
    assert_ne!(edited, original);
    for _ in 0..20 {
        editor.undo().unwrap();
    }
    assert_eq!(editor.document(), &original);
    for _ in 0..20 {
        editor.redo().unwrap();
    }
    assert_eq!(editor.document(), &edited);
    assert_eq!(editor.document().objects[0].vertices[0].id, id);
}

#[test]
fn repeated_save_keeps_previous_revision_and_rejects_stale_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crate.mstrike");
    let mut document = Document::new_primitive(PrimitiveKind::Cube, [1.0, 1.0, 1.0]).unwrap();
    document.save_new(&path).unwrap();
    let first = std::fs::read(&path).unwrap();
    let hash = Document::archive_sha256(&path).unwrap();
    let id = document.objects[0].vertices[0].id;
    document.move_vertex(id, [-0.4, 0.5, 0.5]).unwrap();
    document.save_replace(&path, &hash).unwrap();
    assert_eq!(Document::load(&path).unwrap(), document);
    assert_eq!(
        std::fs::read(Document::previous_path(&path)).unwrap(),
        first
    );
    let second = std::fs::read(&path).unwrap();
    assert!(document.save_replace(&path, &hash).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), second);
}
