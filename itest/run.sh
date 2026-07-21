#!/usr/bin/env bash
# Builds the geodot-rust extension with the `itest` feature, copies the
# resulting dynamic library into `bin/`, and runs the integration tests
# headlessly.
#
# Requires a Godot 4.5+ binary. Override the binary via the GODOT env var:
#   GODOT=/path/to/godot4 ./run.sh
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
godot_bin="${GODOT:-godot4}"

echo "==> Building geodot-rust (feature: itest)"
cargo build --manifest-path "$repo_root/rust/Cargo.toml" --features itest

# Locate the freshly built dylib for the current platform.
target_dir="$repo_root/rust/target/debug"
case "$(uname -s)" in
	Linux*)  lib_name="libgeodot_rust.so" ;;
	Darwin*) lib_name="libgeodot_rust.dylib" ;;
	MINGW* | MSYS* | CYGWIN*) lib_name="geodot_rust.dll" ;;
	*) echo "Unsupported platform: $(uname -s)" >&2; exit 1 ;;
esac

mkdir -p "$script_dir/bin"
cp "$target_dir/$lib_name" "$script_dir/bin/"
echo "==> Copied $lib_name into itest/bin"

echo "==> Running integration tests with $godot_bin"
"$godot_bin" --headless --path "$script_dir" --quit -s run_tests.gd
