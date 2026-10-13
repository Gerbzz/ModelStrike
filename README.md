# ModelStrike

**by PWDS**

**Start with the shape.**

ModelStrike helps creators inspect and reshape simple 3D prop geometry. Open an OBJ or make a basic mesh, check its dimensions and warnings, edit or transform the geometry, and export a new OBJ while leaving the source alone.

## Highlights

- Inspect OBJ geometry and source fingerprints.
- Edit simple triangle meshes and transforms.
- Export a new OBJ for further review.

## Current status

Windows prototype. CLI and native UI code compiled and passed bounded geometry checks, but native pointer use, visible layout, accessibility, clean installation, and Valve-compatible asset round trips remain unverified. This is not a Source 2 model compiler.

## Edit a triangle mesh

The **Mesh editor** opens first in the desktop window; the earlier workflow remains under **OBJ utility**. Create a cube, plane, or cylinder, or import a static OBJ, glTF, or GLB source. A hierarchy and isometric wireframe preview allow vertex or triangle-face selection. Move a vertex, set an object's RGBA base color, or inset and raise one triangle face along its winding normal. These edits are undoable and redoable. The face action retains the selected cap ID and material assignment; it is a local triangle operation, not general polygon modeling or collision repair.

Save and reopen an editable `.mstrike` archive. It stores stable IDs, positions, normals, UV0, simple base-color materials, and a hashed manifest. Saving validates and rereads a temporary archive before no-clobber persistence; repeated saves retain a `.previous.mstrike` sibling. The editor holds up to 100 undo transactions in memory, but has no on-disk command history or autosave. Export static triangulated geometry to a new OBJ or GLB. OBJ does not write an MTL, while GLB keeps base-color factors but not source textures or extended PBR properties. Import warns about features it cannot edit, including animation and skinning.

| File | Role |
| --- | --- |
| Source <code>.obj</code> | Existing mesh or a ModelStrike-generated cube |
| Inspection report <code>.json</code> | Geometry counts, bounds, warnings, and source fingerprint |
| Transform project <code>.json</code> | Source reference and hash plus translation/scale controls |
| Export <code>.obj</code> | Separate transformed geometry |
| Editable <code>.mstrike</code> | Hashed, versioned triangle-mesh document |
| Mesh export <code>.obj</code> or <code>.glb</code> | Separate static triangulated output |

A transform project prefers a source path relative to the project folder when possible. If the OBJ moves or changes, the UI can still reopen saved controls and warn about the source; export remains blocked until the source issue is resolved. Keep the source with the project when moving work.

## Work with a prop source

1. Open an OBJ or create a cube. The generated cube has triangle faces, UV coordinates, and flat face normals.
2. Inspect the mesh. ModelStrike reports counts, bounds, warnings, and a SHA-256 fingerprint of the source.
3. Set a translation in X/Y/Z and a positive uniform scale. Export a new OBJ; the source file is left unchanged.
4. In the desktop UI, save the controls to a versioned JSON transform project and reopen them later.

The transform rewrites vertex positions and keeps the other OBJ lines, including face and UV records. It does not run a general mesh-repair or material-conversion pipeline. New outputs refuse to overwrite an existing destination.

## Working safely with source geometry

The inspection pass reads OBJ vertex and face records, computes bounds and counts, and reports issues it can detect in this limited format. The cube generator writes a small UV- and normal-bearing example mesh for experiments without requiring an external asset.

The transform project stores controls separately from the OBJ. It compares the saved source fingerprint when reopening; a missing or changed source leaves the settings recoverable in the UI but prevents an export from using the wrong geometry. The UI also asks for an explicit decision before replacing unsaved transform controls with another source or project.

The mesh import path checks the same OBJ bytes it passes to the importer. Unsupported concave, self-intersecting, nonplanar, collinear, and float32-collapsed faces are rejected instead of silently fan-triangulated; individual OBJ faces are limited to 1,024 corners. GLB export rejects geometry that would lose required shape fidelity at float32 precision. Independent probes confirmed these rejections leave source files and occupied destinations unchanged for the tested fixtures. These checks do not establish support for arbitrary OBJ meshes or large scenes.

## Build and run

Use Windows and a Rust toolchain that supports edition 2024 and the versions in `Cargo.lock`. The desktop application is first-party Rust with egui/eframe and native library dependencies. The native-feature test checkpoint used `--locked --offline`; a clean-machine build has not been verified.

~~~powershell
cargo test --locked --features native-ui
cargo build --locked --features native-ui --bins
cargo run --locked --features native-ui --bin modelstrike-ui
~~~

The CLI retains inspection, cube creation, and direct transforms. Saving and reopening the older JSON transform project remains a desktop-UI workflow. The editable mesh CLI supports deterministic local files:

~~~powershell
cargo run --locked --bin modelstrike -- inspect <source.obj> <new-report.json>
cargo run --locked --bin modelstrike -- new-cube <edge> <new-cube.obj>
cargo run --locked --bin modelstrike -- transform <source.obj> <new-output.obj> <dx> <dy> <dz> <scale>
cargo run --locked --bin modelstrike -- mesh-new cube 1 0.8 0.6 <new.mstrike>
cargo run --locked --bin modelstrike -- mesh-import <source.obj> <new.mstrike> 1
cargo run --locked --bin modelstrike -- mesh-move-vertex <in.mstrike> <new.mstrike> <vertex-id> <x> <y> <z>
cargo run --locked --bin modelstrike -- mesh-set-color <in.mstrike> <new.mstrike> <object-id> <r> <g> <b>
cargo run --locked --bin modelstrike -- mesh-inset-raise-face <in.mstrike> <new.mstrike> <face-id> <inset-fraction> <raise-metres>
cargo run --locked --bin modelstrike -- mesh-export-glb <in.mstrike> <new.glb>
cargo run --locked --bin modelstrike -- mesh-export-obj <in.mstrike> <new.obj>
~~~

The cube edge and transform values must be finite numbers; edge and scale must be positive.

## Boundaries and direction

ModelStrike does not import or export `.vmdl`, compile Source 2 assets, assign game materials, rig characters, or verify a prop in Counter-Strike. Face editing operates on triangles; it does not support arbitrary polygons, manifold repair, shaded orbit, or collision checks against other surfaces. The native editor uses a wireframe preview. Its pointer behavior, visible layout, keyboard and screen-reader access, and clean-machine installation have not been verified. Linux, macOS, and mobile platforms have not been tested.

Potential next steps include richer mesh, material and collision workflows and a tested Valve-compatible asset path. These are roadmap ideas, not present capabilities.

This repository grants no public license for its first-party project source. Vendored [EffectCraft source and synthetic test fixtures](ATTRIBUTION.md) retain their MIT OR Apache-2.0 licensing and upstream notices in `vendor/effectcraft/`. Other third-party crates retain their own licenses.
