# ModelStrike third-party code

EffectCraft `4b58cd2eb733bdc7ad39161e93dd8e2ff4af6416` by ArtCraft Team and contributors, licensed MIT OR Apache-2.0.

Source and synthetic model test-fixture content from `crates/model` and `crates/geom` at the pinned commit is preserved in `vendor/effectcraft/{model,geom}`. Windows text files use CRLF line endings; normalized content matches upstream. The two crate manifests replace workspace-inherited dependency entries with standalone, semantically equivalent version requirements and a local geometry path. The nested model `Cargo.lock` was generated for standalone vendor testing. Upstream licenses and NOTICE are preserved in `vendor/effectcraft/`. No ArtCraft marks or font assets were copied. The EffectCraft name appears here only as attribution.

The model parser and primitive generator are alpha donor code and must be validated against ModelStrike fixtures before release. Other Rust dependencies retain their own licenses and require a distribution audit before shipping installers.
