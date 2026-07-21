use godot::prelude::*;

mod error;
mod geo_image;
mod geo_transform;
mod global;
mod raster_tile_extractor;
mod vector_extractor;

struct GeodotRust;

#[gdextension]
unsafe impl ExtensionLibrary for GeodotRust {}
