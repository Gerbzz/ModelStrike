# ModelStrike

Prop/model preparation prototype with OBJ inspection, transforms, and project save/reopen. It does not compile a Source 2 model.

## Build and test

On Windows with a Rust 2024 toolchain:

```powershell
cargo test --locked
cargo build --locked --features native-ui --bins
```

The code is a local prototype. Native windows, installers, and full production workflows have not passed clean-machine acceptance. Project source in this private repository has no public license grant. Third-party crates retain their own licenses.
