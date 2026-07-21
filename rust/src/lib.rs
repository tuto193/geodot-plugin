use godot::prelude::*;

mod error;

struct GeodotRust;

#[gdextension]
unsafe impl ExtensionLibrary for GeodotRust {}
