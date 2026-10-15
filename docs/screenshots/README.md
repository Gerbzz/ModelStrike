# ModelStrike renderer capture provenance

`modelstrike-mesh-editor.png` is an unedited 1280 × 820 capture from the running native egui renderer. The repository's `modelstrike-capture` target loads the same `ModelStrikeApp` used by the desktop UI, requests an egui screenshot after its second frame, writes the PNG, and closes. This is renderer evidence, not a record of a person interacting with the full app.

- Source: public `main` commit [`605ae71e6cf89da490a393a7d5de2bf09ac5c6d3`](https://github.com/Gerbzz/ModelStrike/commit/605ae71e6cf89da490a393a7d5de2bf09ac5c6d3); no application source changes were made for the capture.
- Host and date: Office PC, 2026-10-15 UTC. Cargo 1.97.1 and rustc 1.97.1.
- Build: offline and single job, with debug symbols and incremental compilation disabled. C: had 22.32 GiB free before the build and 21.32 GiB after capture, within the 6 GiB build-growth budget.
- Built `modelstrike-capture.exe` SHA-256: `ad2f877cb78d98dc41135a8984bc6a78fe969ae1af308d5b1118a28372735f04`.
- Built `modelstrike.exe` SHA-256: `a2ece9a3a92678784b3e5fa92321f98aa54ca256fbe4fd1d7553904bbefbdce4`.
- [Synthetic cuboid fixture](fixtures/synthetic-cuboid.mstrike) SHA-256: `28c1622f43631f408252145959f906e23165b17cdfb3bafd108eb196df5db11e`. It contains one object, 12 triangles, and bounds X -0.5 to 0.5, Y -0.4 to 0.4, Z -0.3 to 0.3 metres; the first-party CLI created it without outside assets.
- [Captured PNG](modelstrike-mesh-editor.png) SHA-256: `42a580f764cd6adb7b9893676156319d91dc676325664145d42577cb54256283`.

Commands run from the repository root on the pinned source:

```powershell
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
cargo build --locked --offline --features native-ui --bin modelstrike-capture --bin modelstrike -j 1
.\target\debug\modelstrike.exe mesh-new cube 1 0.8 0.6 docs\screenshots\fixtures\synthetic-cuboid.mstrike
.\target\debug\modelstrike.exe mesh-info docs\screenshots\fixtures\synthetic-cuboid.mstrike
.\target\debug\modelstrike-capture.exe docs\screenshots\fixtures\synthetic-cuboid.mstrike docs\screenshots\modelstrike-mesh-editor.png vertex:2
```

The screenshot visibly shows the mesh editor, isometric wireframe, selected vertex 2, bounds, and properties. At this fixed viewport, the lower edge of the wireframe extends below the preview area. The capture does not exercise pointer input, native file dialogs, editing commands, save/reopen interaction, exports, accessibility, other window sizes, or installation.
