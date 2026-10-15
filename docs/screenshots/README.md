# ModelStrike renderer capture provenance

`modelstrike-mesh-editor.png` is an unedited 1280 × 820 capture from the running native egui renderer. The repository's `modelstrike-capture` target loads the same `ModelStrikeApp` used by the desktop UI, requests an egui screenshot after its second frame, writes the PNG, and closes. This is renderer evidence, not a record of a person interacting with the full app.

- Capture source: commit [`39244ec15166ab5587727262ff415bcb534e0def`](https://github.com/Gerbzz/ModelStrike/commit/39244ec15166ab5587727262ff415bcb534e0def). Its narrow layout fix reserves the status bar before allocating the central preview. The first published capture in parent commit `86c4ae1` showed a clipped lower wireframe edge and was superseded.
- Host and date: Office PC, 2026-10-15 UTC. Cargo 1.97.1 and rustc 1.97.1.
- Build: offline and single job, with debug symbols and incremental compilation disabled. The capture target was rebuilt from `39244ec` using cached dependencies. C: had 22.32 GiB free before the initial build and 21.09 GiB after the fix and recapture, within the 6 GiB build-growth budget.
- Rebuilt `modelstrike-capture.exe` SHA-256: `adb6e5145411bc86e4f8ad51e56175d483221cb380e51aa2d9ac7edce47f5b13`.
- The unchanged fixture-generating `modelstrike.exe` was built from source commit `605ae71e6cf89da490a393a7d5de2bf09ac5c6d3`; SHA-256: `a2ece9a3a92678784b3e5fa92321f98aa54ca256fbe4fd1d7553904bbefbdce4`.
- [Synthetic cuboid fixture](fixtures/synthetic-cuboid.mstrike) SHA-256: `28c1622f43631f408252145959f906e23165b17cdfb3bafd108eb196df5db11e`. It contains one object, 12 triangles, and bounds X -0.5 to 0.5, Y -0.4 to 0.4, Z -0.3 to 0.3 metres; the first-party CLI created it without outside assets.
- [Captured PNG](modelstrike-mesh-editor.png) SHA-256: `d271e981fc3c30920192e0bae2aa09d8e338a7ee936fdc9ac83b34896e5957a4`.

Commands run from the repository root. The fixture and CLI were built at `605ae71`; the final capture target was rebuilt after the layout fix at `39244ec`:

```powershell
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
cargo build --locked --offline --features native-ui --bin modelstrike-capture --bin modelstrike -j 1
.\target\debug\modelstrike.exe mesh-new cube 1 0.8 0.6 docs\screenshots\fixtures\synthetic-cuboid.mstrike
.\target\debug\modelstrike.exe mesh-info docs\screenshots\fixtures\synthetic-cuboid.mstrike
# At source commit 39244ec, after its regression test passed:
cargo build --locked --offline --features native-ui --bin modelstrike-capture -j 1
.\target\debug\modelstrike-capture.exe docs\screenshots\fixtures\synthetic-cuboid.mstrike target\fixed-capture.png vertex:2
Copy-Item -LiteralPath target\fixed-capture.png -Destination docs\screenshots\modelstrike-mesh-editor.png -Force
```

The screenshot visibly shows the mesh editor, the full isometric wireframe within its preview, selected vertex 2, bounds, and properties. A headless egui regression first reproduced the old preview bottom at y=812 in an 820-pixel window; after the panel-order fix it passed. The full native-feature suite passed 41 tests with `TEMP` and `TMP` set to a writable `target/test-tmp` directory. The default sandbox temp location caused unrelated file-writing tests to fail with access denied before that targeted rerun. The capture does not exercise pointer input, native file dialogs, editing commands, save/reopen interaction, exports, accessibility, other window sizes, or installation.
