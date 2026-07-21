mod coordinate_transform;
mod features;
mod native_dataset;
mod native_layer;

pub use coordinate_transform::CoordinateTransform;
pub use features::{
    DynFeatureHolder, Feature, GeometryType, LineFeature, PointFeature, PolygonFeature,
    UnknownFeature,
};
pub use native_dataset::NativeDataset;
pub use native_layer::NativeLayer;

use std::path::PathBuf;

use gdal::errors::GdalError;

/// Top-level entry point for the vector side of the plugin.
///
/// Mirrors the C++ `VectorExtractor` façade: it initializes GDAL and opens
/// datasets, handing back the higher-level [`NativeDataset`] wrapper.
pub struct VectorExtractor;

impl VectorExtractor {
    /// Registers all GDAL drivers. Must be called before any other function to
    /// initialize GDAL.
    // Part of the intended public/FFI API surface; not called from within the crate.
    #[allow(dead_code)]
    pub fn initialize() {
        gdal::DriverManager::register_all();
    }

    /// Opens the dataset at the given path and returns a [`NativeDataset`], or a
    /// [`GdalError`] if it could not be opened.
    // Part of the intended public/FFI API surface; not called from within the crate.
    #[allow(dead_code)]
    pub fn open_dataset(path: &PathBuf, write_access: bool) -> Result<NativeDataset, GdalError> {
        NativeDataset::new(path, write_access)
    }
}
