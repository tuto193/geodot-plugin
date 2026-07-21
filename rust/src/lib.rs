use godot::prelude::*;

mod error;
mod geo_data;
mod geo_features;
mod geo_image;
mod geo_transform;
mod global;
#[cfg(feature = "itest")]
mod itest;
mod raster_tile_extractor;
mod vector_extractor;

struct GeodotRust;

#[gdextension]
unsafe impl ExtensionLibrary for GeodotRust {}
