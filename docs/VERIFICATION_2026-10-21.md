# BlendStrike verification checkpoint - 2026-10-21

## Source and isolation

- Repository: `https://github.com/Gerbzz/ModelStrike.git`; display name BlendStrike. Remote unchanged.
- Inspected main: `ba5dc65e0c78c3c5a6a21e9edd9f7e77d939d908`.
- Room PC: `DESKTOP-A4P8DV3`. Isolated checkout: `Documents/Codex/2026-10-21/task-4/BlendStrike`.
- Working branch: `codex/blendstrike-save-recovery-20261021`; [draft PR #2](https://github.com/Gerbzz/ModelStrike/pull/2). No merge or release.
- Local Git comparison `git diff --exit-code main 4b136beb5a536d58afe5da13df72d95b83f8904f -- .` returned 0: main has exactly the previously tested source tree.

## Tests and investigation

The prior [Windows run 38988162736](https://github.com/Gerbzz/ModelStrike/actions/runs/38988162736), job `117264349576`, passed 41 tests with `cargo test --locked --features native-ui`, then built the capture target. This evidence was reread rather than treated as a new local run.

An initial hypothesis was that saving a reopened `crate.previous.mstrike` could overwrite its own recovery archive. Two new integration tests were written before any production change and run against the unchanged implementation. Both passed. Rust's `with_extension("previous.mstrike")` replaces only the final extension, so it already derives the distinct `crate.previous.previous.mstrike` path. The hypothesis was **disproved**, and no production fix was made.

The added tests use actual temporary `.mstrike` archives, not mocked persistence:

- `saving_a_reopened_previous_revision_keeps_a_distinct_recovery_archive`: edit/save a recovery file; load both versions; compare backup bytes to the original; reject a stale save while preserving current and backup bytes.
- `successive_recovery_files_each_keep_their_own_previous_revision`: save twice for ordinary, previous and nested previous filenames; ensure distinct backup paths and byte-exact preceding revisions.

Fresh [Windows run 39557809604](https://github.com/Gerbzz/ModelStrike/actions/runs/39557809604), job `119081616503`, source `6ea2b152813fb040bd71e89cdf56150f9d413479`, completed successfully. It ran `cargo test --locked --features native-ui`: **43 passed, 0 failed** (13 library/native-feature, 4 edit history, 8 editable workflow, 5 face edit, 10 geometry validation, 3 material). It also built `modelstrike-capture`. The final documentation/formatting head is checked by the same PR workflow; its exact head and final run are recorded in the PR description and local handoff.

No reproducible import/save integrity defect was established in this bounded review. This does not certify arbitrary files, concurrent writers, all donor code, crash durability, large scenes or Blender compatibility. The concrete source advance is additional recovery-save coverage and the [migration boundary](SOURCE_MIGRATION_BOUNDARY.md).

## Native and environment limits

The Room-PC suite handoff says Smart App Control blocks local Rust builds. No local cargo build/test, downloaded executable, application launch, file-unblocking, elevation of an app, security-policy change or alternate execution path was attempted. Source testing ran on GitHub's hosted Windows runner. It checks implementation behavior and headless UI logic, not a person using the native app.

No fresh screenshot was obtained. The committed screenshot's own provenance records an older Office-PC render at `39244ec`; the current hierarchy still lacks fresh native/input acceptance. That computer was not accessed.

C: free space was over 2,063 GiB at the local checkpoint, above the 20 GiB floor. The source checkout excluding Git metadata occupied approximately 1.5 MB. No heavy local or upstream build, package installation, branding edit, repository rename, merge, deployment or release occurred.

## Remaining decision

A complete upstream Blender foundation is still unselected. The migration document gives evidence for a full preserved Blender fork versus a Blender-backed template/extension path. Both retain Blender's real data/tools rather than claiming that more custom triangle-editor features provide Blender parity. Select the source/distribution boundary before acquiring/building the full foundation; coordinate a heavy build separately.
