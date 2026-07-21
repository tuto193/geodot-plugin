# geodot-rust integration tests

Some of the Godot-facing wrapper classes (`GeoImage`, `GeoRasterLayer`,
`GeoFeatureLayer`, the `Geo*` feature types, ...) construct Godot builtins or
instantiate `GodotClass` objects. Those calls panic outside of an initialized
Godot engine, so they cannot be covered by the plain `cargo test` unit tests.

This directory contains a tiny headless Godot project that loads the Rust
extension (built with the `itest` cargo feature) and drives an in-engine test
runner defined in [`../rust/src/itest.rs`](../rust/src/itest.rs).

## Requirements

- A Godot **4.5+** binary on your `PATH` as `godot4` (or point the `GODOT`
  environment variable at it). Users of the nix flake can add `godot4` there.
- The datasets in [`../demo/geodata`](../demo/geodata) (already in the repo).

## Running

```sh
./run.sh
```

This builds `geodot-rust` with `--features itest`, copies the resulting dynamic
library into `bin/`, and runs the tests headlessly. The process exits with code
`0` if all tests pass and `1` otherwise, so it can be wired directly into CI.

To run manually after building:

```sh
GODOT=/path/to/godot4 ./run.sh
# or, if the dylib is already in bin/:
godot4 --headless --path . --quit -s run_tests.gd
```

## Adding tests

Add a new `fn(&GeoData) -> Result<(), String>` in `rust/src/itest.rs` and
register it in the `TESTS` array. Return `Err(reason)` to fail a test.
