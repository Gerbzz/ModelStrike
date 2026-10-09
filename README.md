# ModelStrike

**A native Rust prop-source workshop for inspecting and transforming OBJ meshes.** ModelStrike gives a creator a small starting shape, a way to check incoming geometry, and repeatable translation and scale controls before exporting a separate OBJ. It is a preparation prototype for custom-map props, not a Source 2 model compiler.

> **Current stage:** OBJ inspection, cube generation, vertex transforms, a saved transform project, CLI commands, and the desktop window are implemented and headlessly tested. Valve-compatible asset round trips and visible window review remain open.

## Work with a prop source

1. Open an OBJ or create a cube. The generated cube has triangle faces, UV coordinates, and flat face normals.
2. Inspect the mesh. ModelStrike reports counts, bounds, warnings, and a SHA-256 fingerprint of the source.
3. Set a translation in X/Y/Z and a positive uniform scale. Export a new OBJ; the source file is left unchanged.
4. In the desktop UI, save the controls to a versioned JSON transform project and reopen them later.

The transform rewrites vertex positions and keeps the other OBJ lines, including face and UV records. It does not run a general mesh-repair or material-conversion pipeline. New outputs refuse to overwrite an existing destination.

| File | Role |
| --- | --- |
| Source <code>.obj</code> | Existing mesh or a ModelStrike-generated cube |
| Inspection report <code>.json</code> | Geometry counts, bounds, warnings, and source fingerprint |
| Transform project <code>.json</code> | Source reference and hash plus translation/scale controls |
| Export <code>.obj</code> | Separate transformed geometry |

A transform project prefers a source path relative to the project folder when possible. If the OBJ moves or changes, the UI can still reopen saved controls and warn about the source; export remains blocked until the source issue is resolved. Keep the source with the project when moving work.

## Working safely with source geometry

The inspection pass reads OBJ vertex and face records, computes bounds and counts, and reports issues it can detect in this limited format. The cube generator writes a small UV- and normal-bearing example mesh for experiments without requiring an external asset.

The transform project stores controls separately from the OBJ. It compares the saved source fingerprint when reopening; a missing or changed source leaves the settings recoverable in the UI but prevents an export from using the wrong geometry. The UI also asks for an explicit decision before replacing unsaved transform controls with another source or project.

## Build and run

Use Windows and a Rust toolchain with edition 2024 support. The desktop application is first-party Rust with egui/eframe and native library dependencies.

~~~powershell
cargo test --locked --features native-ui
cargo build --locked --features native-ui --bins
cargo run --locked --features native-ui --bin modelstrike-ui
~~~

The CLI covers inspection, cube creation, and direct transforms. Saving and reopening transform projects is a desktop-UI workflow in this published revision.

~~~powershell
cargo run --locked --bin modelstrike -- inspect <source.obj> <new-report.json>
cargo run --locked --bin modelstrike -- new-cube <edge> <new-cube.obj>
cargo run --locked --bin modelstrike -- transform <source.obj> <new-output.obj> <dx> <dy> <dz> <scale>
~~~

The cube edge and transform values must be finite numbers; edge and scale must be positive.

## Boundaries and direction

ModelStrike does not import or export <code>.vmdl</code>, compile Source 2 assets, assign game materials, rig characters, or verify a prop in Counter-Strike. Its OBJ operations are a narrow local workflow. A clean-machine installer and visual workflow pass have not been completed.

Potential next steps include richer primitive and mesh tools, material and collision workflows, and a tested Valve-compatible asset path. These are roadmap ideas, not present capabilities.

This private repository grants no public license for its project source. Third-party crates retain their own licenses.
