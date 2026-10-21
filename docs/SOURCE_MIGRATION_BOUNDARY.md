# BlendStrike source migration boundary

Reviewed 2026-10-21 against ModelStrike main `ba5dc65e0c78c3c5a6a21e9edd9f7e77d939d908`.
BlendStrike is the display name; the GitHub repository and remote remain `Gerbzz/ModelStrike`.
This records an architecture boundary and an unresolved foundation decision, not a Blender implementation or parity claim.

## What the current source actually preserves

`ATTRIBUTION.md` identifies EffectCraft `4b58cd2eb733bdc7ad39161e93dd8e2ff4af6416` as the donor. It preserves the `model` and `geom` crates, synthetic fixtures, and their notices under `vendor/effectcraft/`, with standalone manifest adaptations. This is a selected model/geometry subset, not a complete upstream application checkout. The attribution does not identify a canonical upstream repository URL; that provenance must be resolved before expanding or refreshing the donor snapshot.

`Cargo.toml` builds a first-party Rust application using those local crates and optional egui/eframe. `src/editor.rs` converts imported instances into stable-ID triangle meshes. Its document stores positions, normals, UV0, triangle material IDs and base-color slots in a hashed two-entry ZIP archive. Undo is session-local snapshots. `src/ui/mesh.rs` operates this custom document through an isometric wireframe and vertex/triangle tools. `src/project.rs` separately manages the older OBJ transform-reference documents.

The import boundary flattens instances into world-space mesh objects. It drops the editable source hierarchy and source animation/rigging behavior; warnings cover animation, skins, textures, cameras and lights. The document has no general polygon topology, modifier/dependency graph, native `.blend` database, node system or Blender operator integration. OBJ/GLB exports are static interchange outputs. Expanding this document does not incrementally migrate Blender's implementation.

## Evidence for the missing foundation

The official [Blender mirror README](https://github.com/blender/blender/blob/63e2be8faaaa88994017083aec0a35b6f30392b0/README.md) describes a complete 3D creation suite and its whole-program GPLv3 license. The following source paths were verified through the public GitHub API on 2026-10-21. Commit `63e2be8faaaa88994017083aec0a35b6f30392b0` is a reference examined for this assessment, **not a selected release or adopted dependency**.

| Concern | Current repository | Blender source boundary |
| --- | --- | --- |
| Editable topology | Custom triangles with vertex IDs | [`source/blender/bmesh`](https://github.com/blender/blender/tree/63e2be8faaaa88994017083aec0a35b6f30392b0/source/blender/bmesh) |
| Scene data and native persistence | Small JSON mesh document inside `.mstrike` | [`blenkernel`](https://github.com/blender/blender/tree/63e2be8faaaa88994017083aec0a35b6f30392b0/source/blender/blenkernel), `makesdna`, `makesrna`, `blenloader` |
| Evaluation and creation tools | Snapshot undo and a few local triangle operations | [`depsgraph`](https://github.com/blender/blender/tree/63e2be8faaaa88994017083aec0a35b6f30392b0/source/blender/depsgraph), `modifiers`, `nodes`, `editors` |
| UI/API integration | egui/eframe custom UI | [`scripts/modules/bpy`](https://github.com/blender/blender/blob/63e2be8faaaa88994017083aec0a35b6f30392b0/scripts/modules/bpy/__init__.py), `windowmanager`, `editors` |

The repository contains none of that Blender source or its build tree. The current README also gives no public license grant for the first-party source; the Blender README and [COPYING](https://github.com/blender/blender/blob/63e2be8faaaa88994017083aec0a35b6f30392b0/COPYING) identify GPL licensing. Source ownership and the intended distribution model therefore belong in the foundation decision, before combining implementations. No licensing decision is made here.

## Two concrete choices requiring the user's direction

1. **Full Blender source fork.** Preserve a complete, immutable official release source checkout plus required build/dependency provenance, licenses and upstream history. Keep Blender's data model, persistence, evaluation and modeling tools authoritative. Incrementally change product behavior and UI in reviewable commits, with comparisons against the untouched pinned upstream. This most directly matches the requirement to preserve a complete selected source and migrate it; it requires a maintained C/C++/Python foundation, a distribution decision and coordinated build capacity.
2. **Blender-based application template or extension first.** Preserve and pin the complete selected Blender source separately and keep Blender's runtime/data authoritative while implementing a bounded BlendStrike workflow through `bpy` and scripts. The verified `bpy` module exposes data, context, operators, types and utilities and initializes scripts/app-template support. This can test a creation workflow before maintaining a deep core fork, but it remains dependent on Blender and does not turn the present Rust editor into Blender. Distribution and the eventual core-fork boundary still need a decision.

Continuing the custom editor alone remains a bounded mesh utility and does not satisfy either source-preserving Blender path. No upstream was selected, cloned, built, installed or launched in this checkpoint.

## Work that is safe before that decision

- Keep the existing `.mstrike` schema, CLI and importer/exporter contracts stable; repair independently reproduced integrity defects with regressions.
- Preserve current source fixtures and branding. Keep the existing custom document explicitly a legacy/interchange format; do not represent it as a `.blend` replacement or silently make it the future authoritative scene model.
- Once a foundation is selected, record canonical upstream URL, release/tag and resolved commit, complete-source hash/manifest, dependencies and licenses. Preserve the untouched full source before adapting it. Keep migration changes separate from the upstream snapshot and inventory every deliberate divergence.
- Define one vertical acceptance case against pinned upstream: create a mesh, edit it, undo/redo, save/reopen its native scene, and verify topology/material/scene data. Add import/export comparisons and real native-input evidence. Only claim capabilities demonstrated by that source and those checks.
- Coordinate any heavy upstream build first and retain at least 20 GiB free. Room-PC application-control restrictions remain in force; no alternative execution route or security change is authorized here.

## Current evidence limit

The successful Windows CI run [38988162736](https://github.com/Gerbzz/ModelStrike/actions/runs/38988162736) tested head `4b136beb5a536d58afe5da13df72d95b83f8904f`: 41 native-feature tests passed. A local Git tree comparison confirms it is identical to the inspected merged main tree. Those tests exercise source behavior and headless UI logic, not pointer/file-dialog acceptance.

The committed renderer screenshot predates the latest hierarchy changes and its provenance identifies an Office-PC capture at `39244ec`. It is historical evidence only. This checkpoint did not access that computer, launch an executable, obtain a fresh native screenshot, or validate the current layout through native input.
