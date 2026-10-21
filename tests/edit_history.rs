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

#[test]
fn saving_a_reopened_previous_revision_keeps_a_distinct_recovery_archive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crate.previous.mstrike");
    let backup = dir.path().join("crate.previous.previous.mstrike");
    let original = Document::new_primitive(PrimitiveKind::Cube, [1.0; 3]).unwrap();
    original.save_new(&path).unwrap();
    let original_bytes = std::fs::read(&path).unwrap();
    let hash = Document::archive_sha256(&path).unwrap();
    let mut edited = Document::load(&path).unwrap();
    let id = edited.objects[0].vertices[0].id;
    edited.move_vertex(id, [-0.4, 0.5, 0.5]).unwrap();
    edited.save_replace(&path, &hash).unwrap();

    assert_eq!(Document::load(&path).unwrap(), edited);
    assert_eq!(Document::load(&backup).unwrap(), original);
    assert_eq!(std::fs::read(&backup).unwrap(), original_bytes);
    let saved_bytes = std::fs::read(&path).unwrap();
    assert!(edited.save_replace(&path, &hash).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved_bytes);
    assert_eq!(std::fs::read(&backup).unwrap(), original_bytes);
}

#[test]
fn successive_recovery_files_each_keep_their_own_previous_revision() {
    let dir = tempfile::tempdir().unwrap();
    let original = Document::new_primitive(PrimitiveKind::Cube, [1.0; 3]).unwrap();
    for name in ["crate.mstrike", "crate.previous.mstrike", "crate.previous.previous.mstrike"] {
        let iteration = tempfile::tempdir_in(dir.path()).unwrap();
        let path = iteration.path().join(name);
        original.save_new(&path).unwrap();
        let first = std::fs::read(&path).unwrap();
        let mut edited = original.clone();
        let id = edited.objects[0].vertices[0].id;
        edited.move_vertex(id, [-0.4, 0.5, 0.5]).unwrap();
        edited.save_replace(&path, &Document::archive_sha256(&path).unwrap()).unwrap();
        let backup = Document::previous_path(&path);
        assert_ne!(backup, path, "{name}");
        assert_eq!(std::fs::read(&backup).unwrap(), first, "{name}");
        let second = std::fs::read(&path).unwrap();
        let hash = Document::archive_sha256(&path).unwrap();
        edited.move_vertex(id, [-0.3, 0.5, 0.5]).unwrap();
        edited.save_replace(&path, &hash).unwrap();
        assert_eq!(Document::load(&path).unwrap(), edited);
        assert_eq!(std::fs::read(&backup).unwrap(), second, "{name}");
    }
}

