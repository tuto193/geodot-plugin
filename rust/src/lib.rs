use godot::prelude::*;

mod error;
mod global;

struct GeodotRust;

#[gdextension]
unsafe impl ExtensionLibrary for GeodotRust {}
