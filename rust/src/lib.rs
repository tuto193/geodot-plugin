use godot::prelude::*;

mod error;
mod geo_transform;
mod global;
mod vector_extractor;

struct GeodotRust;

#[gdextension]
unsafe impl ExtensionLibrary for GeodotRust {}
