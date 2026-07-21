extends SceneTree

# Headless entry point for the geodot-rust integration tests.
#
# Instantiates the Rust `GeodotItest` runner (registered by the loaded
# GDExtension) and runs every test against the datasets bundled in
# `../demo/geodata`. The process exit code is 0 on success and 1 on any
# failure, so this script can be used directly in CI.

func _init() -> void:
	# The geodata lives in the sibling `demo/` project. globalize_path turns the
	# project-relative `res://` path into an absolute filesystem path that GDAL
	# can open.
	var geodata_path := ProjectSettings.globalize_path("res://").path_join("../demo/geodata").simplify_path()

	if not ClassDB.class_exists("GeodotItest"):
		push_error("GeodotItest class not found. Was the extension built with `--features itest`?")
		quit(1)
		return

	var runner = ClassDB.instantiate("GeodotItest")
	var success: bool = runner.run_all(geodata_path)

	quit(0 if success else 1)
