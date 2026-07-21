# Builds the `geodot-rust` GDExtension (a `cdylib`) with crane, reusing a single
# `cargoArtifacts` (the compiled dependency closure) across the dependency-only
# build, clippy, fmt, coverage and the final crate build for *one* target.
#
# This function is deliberately consumed via `callPackage` (see flake.nix) so
# that Nixpkgs' dependency *splicing* does the right thing when cross-compiling:
#   - `nativeBuildInputs` (pkg-config, bindgen/libclang) come from the *build*
#     platform (they have to run on the machine doing the compiling), while
#   - `buildInputs` (GDAL, libiconv) come from the *target* (host) platform.
# That is exactly why we never hard-code a concrete `pkgs` in here.
{
  lib,
  stdenv,
  pkg-config,
  rustPlatform,
  gdal,
  libiconv,
  # Passed in explicitly from the flake (a crane lib already bound to the right
  # package set + toolchain) plus the pre-cleaned crate source.
  craneLib,
  src,
}:
let
  isCross = stdenv.buildPlatform != stdenv.hostPlatform;

  # Rust target triple for whatever we are building *for*, derived from the
  # package set itself, e.g. "aarch64-apple-darwin", "x86_64-pc-windows-gnu".
  rustTarget = stdenv.hostPlatform.rust.rustcTarget;

  # Cargo wants the linker override in SCREAMING_SNAKE_CASE, e.g.
  # CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER.
  targetEnv = lib.toUpper (builtins.replaceStrings [ "-" ] [ "_" ] rustTarget);

  # Args shared by *every* derivation for this target. They MUST be identical
  # between `buildDepsOnly` and `buildPackage`, otherwise the dependency
  # artifacts won't be reused (Cargo keys its build cache on these).
  commonArgs = {
    inherit src;
    pname = "geodot-rust";
    version = "0.1.0";
    strictDeps = true;

    # The crate has no runnable unit tests yet, and a GDExtension cdylib can't
    # be exercised without a Godot runtime. We therefore never run tests as
    # part of the package build, and especially not when cross-compiling
    # (we can't execute target binaries on the build host). Add a dedicated
    # `cargoNextest`/`cargoTarpaulin` check instead once tests exist.
    doCheck = false;

    # `pkg-config` finds the (target) GDAL. `bindgenHook` exports LIBCLANG_PATH
    # and BINDGEN_EXTRA_CLANG_ARGS for both the gdal-sys *and* gdext bindgen
    # passes, including the correct sysroot when cross-compiling.
    nativeBuildInputs = [
      pkg-config
      rustPlatform.bindgenHook
    ];

    buildInputs = [ gdal ] ++ lib.optionals stdenv.hostPlatform.isDarwin [ libiconv ];

    # Pin the Cargo target explicitly so the dependency build and the crate
    # build land in the same `target/<triple>/…` directory and share cache.
    CARGO_BUILD_TARGET = rustTarget;
  }
  // lib.optionalAttrs isCross {
    # Link with the cross C compiler shipped by the build platform.
    "CARGO_TARGET_${targetEnv}_LINKER" = "${stdenv.cc}/bin/${stdenv.cc.targetPrefix}cc";
  };

  # Compile *only* the dependency closure once. Everything else for this target
  # reuses it.
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  # Re-exported so the flake can build clippy/fmt/coverage that reuse the cache.
  inherit commonArgs cargoArtifacts craneLib;

  package = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
}
