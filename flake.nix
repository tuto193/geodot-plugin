{
  description = "geodot-rust — a GDAL-powered Godot GDExtension, built and cross-compiled with crane";

  # ---------------------------------------------------------------------------
  # Cheat sheet (note: there is no `nix flake build` / `nix flake crossbuild`;
  # those aren't real Nix subcommands — you build *named packages* instead):
  #
  #   nix build                          # native crate for the current host
  #                                      #   (packages.<system>.default)
  #   nix build .#crossbuild             # every feasible cross target for this host
  #                                      #   (empty no-op where GDAL can't cross,
  #                                      #    e.g. from macOS — see crossPackages)
  #   nix build .#x86_64-apple-darwin    # one specific cross target
  #   nix flake check                    # rustfmt + clippy (+ coverage on Linux)
  #   nix develop                        # dev shell
  #
  # After editing inputs, run `nix flake lock` once to record crane + flake-utils.
  # ---------------------------------------------------------------------------

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    # Dev Shell
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Cross-compiling + re-using artifacts
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      fenix,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      this_host_system:
      let
        lib = nixpkgs.lib;

        pkgs = import nixpkgs {
          system = this_host_system;
          # In case we want to allow unfree stuff
          # config.allowUnfreePredicate =
          #   pkg: builtins.elem (lib.getName pkg) [ "1password-cli" ];
        };

        # A toolchain (carrying every target from rust-toolchain.toml) that runs
        # on the build host but can emit code for each target. This is a
        # *function* of `pkgs` on purpose: crane needs the function form to
        # splice a build-host toolchain when cross-compiling — a fixed
        # derivation can't be spliced and gets misread as a *target* binary.
        rustToolchainFor =
          p:
          fenix.packages.${p.stdenv.buildPlatform.system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
	    sha256 = "sha256-h+t2xTBz5yt2YIO+1VMIIGlCU7gyp2LYOFvaV1nwOXU=";
          };

        # Clean the crate source exactly once and reuse it for the native build
        # and every cross build (the result is platform-independent).
        craneLibNative = (crane.mkLib pkgs).overrideToolchain rustToolchainFor;
        src = craneLibNative.cleanCargoSource ./rust;

        # Build the full per-target pipeline (deps + crate) for a given package
        # set. `callPackage` is what makes splicing — and therefore cross
        # compilation — work; see the header comment in ./myCrate.nix.
        pipelineFor =
          targetPkgs:
          let
            craneLib = (crane.mkLib targetPkgs).overrideToolchain rustToolchainFor;
          in
          targetPkgs.callPackage ./myCrate.nix { inherit craneLib src; };

        # --- Native (this host) -------------------------------------------------
        native = pipelineFor pkgs;

        # --- Cross targets ------------------------------------------------------
        # Map of "rust target triple" -> "Nixpkgs crossSystem config triple".
        # Only the entries that are actually buildable *from this host* are
        # listed. The hard part is GDAL (a big C++ lib), not Rust, so the matrix
        # is limited by what Nixpkgs can cross-build:
        #   * windows-msvc CANNOT be produced on Nix -> use windows-gnu (MinGW),
        #     which matches the project's existing Docker/MinGW CI.
        #   * darwin<->linux cross is not feasible on Nix -> run those targets on
        #     a Linux builder/CI (your GitHub workflows already split per-OS).
        crossConfigsByHost = {
          "aarch64-darwin" = {
            "x86_64-apple-darwin" = "x86_64-apple-darwin";
          };
          "x86_64-darwin" = {
            "aarch64-apple-darwin" = "aarch64-apple-darwin";
          };
          "x86_64-linux" = {
            "aarch64-unknown-linux-gnu" = "aarch64-unknown-linux-gnu";
            "x86_64-pc-windows-gnu" = "x86_64-w64-mingw32";
          };
          "aarch64-linux" = {
            "x86_64-unknown-linux-gnu" = "x86_64-unknown-linux-gnu";
            "x86_64-pc-windows-gnu" = "x86_64-w64-mingw32";
          };
        };
        crossConfigs = crossConfigsByHost.${this_host_system} or { };

        mkCrossPkgs =
          crossConfig:
          import nixpkgs {
            localSystem = this_host_system;
            crossSystem = {
              config = crossConfig;
            };
          };

        # rust-target-name -> built cdylib for that target, keeping only the
        # targets that actually *instantiate* on this host. GDAL is not
        # cross-friendly in Nixpkgs (it always pulls in numpy -> meson, which
        # needs a target emulator, and it forces its own test-suite to run), so
        # e.g. from macOS this set is empty: build Linux/Windows targets on a
        # Linux builder/CI instead (your GitHub workflows already do native
        # per-OS builds). The wiring below is correct and "lights up"
        # automatically wherever a target can be cross-compiled.
        instantiates = pkg: (builtins.tryEval (builtins.seq pkg.drvPath true)).success;
        crossPackages = lib.filterAttrs (_name: instantiates) (
          lib.mapAttrs (_name: cfg: (pipelineFor (mkCrossPkgs cfg)).package) crossConfigs
        );

        # Aggregate every cross target into one derivation, each under its own
        # subdirectory (avoids file-name collisions between targets).
        crossbuild = pkgs.runCommand "geodot-rust-crossbuild" { } ''
          mkdir -p "$out"
          ${lib.concatStringsSep "\n" (
            lib.mapAttrsToList (name: pkg: ''cp -r ${pkg} "$out/${name}"'') crossPackages
          )}
        '';
      in
      {
        # `nix build` (default) and `nix build .#<rust-target>` / `.#crossbuild`.
        packages = {
          default = native.package;
          inherit crossbuild;
        }
        // crossPackages;

        # `nix flake check`: lint + format (+ coverage on Linux). Each of these
        # reuses `native.cargoArtifacts`, so dependencies are compiled once.
        checks = {
          crate = native.package;

          clippy = native.craneLib.cargoClippy (
            native.commonArgs
            // {
              inherit (native) cargoArtifacts;
              # crane already injects `--release --locked`; only add the rest.
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            }
          );

          fmt = native.craneLib.cargoFmt { inherit (native.commonArgs) src pname; };
        }
        // lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          # Tarpaulin is x86_64-Linux only; keep it off the macOS dev loop.
          coverage = native.craneLib.cargoTarpaulin (
            native.commonArgs // { inherit (native) cargoArtifacts; }
          );
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            bacon
            gdal
            clang
            clang-tools
            llvmPackages.bintools
            mold
            scons
            pkg-config
            (rustToolchainFor pkgs)
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.gdal ];
          # gdal (bindgen feature) and gdext both run bindgen, which needs libclang.
          LIBCLANG_PATH = lib.makeLibraryPath [ pkgs.llvmPackages.libclang.lib ];
        };
      }
    );
}
