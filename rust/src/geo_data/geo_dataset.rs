use godot::prelude::*;
use std::path::Path;

use super::{GeoFeatureLayer, GeoRasterLayer};
use crate::vector_extractor::NativeDataset;
use crate::vector_extractor::VectorExtractor;

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoDataset {
    base: Base<RefCounted>,
    dataset: Option<NativeDataset>,
    name: GString,
    // #[var]  // This might not need to be available to Godot.
    write_access: bool,
}

#[godot_api]
impl GeoDataset {
    /// Returns true if the GeoDataset could successfully be loaded.
    #[func]
    pub fn is_valid(&self) -> bool {
        self.dataset
            .as_ref()
            .is_some_and(|dataset| dataset.is_valid())
    }

    pub fn get_dataset(&self) -> Option<&NativeDataset> {
        self.dataset.as_ref()
    }

    /// Returns information about this file, i.e. the filename and the path
    #[func]
    pub fn get_file_info(&self) -> Dictionary<GString, GString> {
        let mut result = Dictionary::new();

        let Some(dataset) = self.dataset.as_ref() else {
            return result;
        };
        if !dataset.is_valid() {
            return result;
        }

        let _ = result.insert("name", &self.name);
        let _ = result.insert("path", dataset.path.to_str().unwrap_or_default());
        result
    }

    /// Returns true for read-write-access and false for read-only-access.
    #[func]
    pub fn has_write_access(&self) -> bool {
        self.write_access
    }

    /// Return all GeoRasterLayers objects for this dataset.
    #[func]
    pub fn get_raster_layers(&self) -> Array<Gd<GeoRasterLayer>> {
        let mut result = Array::new();
        let Some(dataset) = self.dataset.as_ref() else {
            return result;
        };
        if !dataset.is_valid() {
            return result;
        }
        if let Some(layers) = dataset.get_raster_layer_names() {
            layers.iter().for_each(|layer_name| {
                let mut to_add = GeoRasterLayer::new_gd();
                to_add.bind_mut().set_name(layer_name.into());
                result.push(to_add.to_godot());
            });
        }
        result
    }

    /// Return all GeoFeatureLayer objects for this dataset.
    #[func]
    pub fn get_feature_layers(&self) -> Array<Gd<GeoFeatureLayer>> {
        let mut result = Array::new();
        if let Some(ref dataset) = self.dataset
            && let Some(names) = dataset.get_feature_layer_names()
        {
            names.iter().for_each(|name| {
                if let Some(layer) = self.get_feature_layer(name.into()) {
                    result.push(layer.to_godot());
                }
            });
        }

        result
    }

    /// Returns a GeoRasterLayer object of the layer within this dataset with
    /// the given name. It is recommended to check the validity of the returned
    /// object with GeoRasterLayer::is_valid().
    #[func]
    pub fn get_raster_layer(&self, name: GString) -> Option<Gd<GeoRasterLayer>> {
        if let Some(ref dataset) = self.dataset {
            if !dataset.is_valid() {
                return None;
            }
            if let Some(subdataset) = dataset.get_subdataset(&name.clone().into()) {
                if !subdataset.is_valid() {
                    return None;
                }
                let mut result = GeoRasterLayer::new_gd();
                result.bind_mut().set_name(name);
                result.bind_mut().set_native_dataset(subdataset);
                result.bind_mut().set_origin_dataset(self.to_gd());
                return Some(result);
            }
        }
        None
    }

    /// Returns a GeoFeatureLayer object of the layer within this dataset with
    /// the given name. It is recommended to check the validity of the returned
    /// object with GeoFeatureLayer::is_valid().
    #[func]
    pub fn get_feature_layer(&self, name: GString) -> Option<Gd<GeoFeatureLayer>> {
        let name = name.into();
        if let Some(ref dataset) = self.dataset {
            if !dataset.is_valid() {
                return None;
            }
            if let Some(layer) = dataset.get_layer(&name) {
                let mut result = GeoFeatureLayer::new_gd();
                result.bind_mut().set_name(name.to_godot());
                result.bind_mut().set_native_layer(layer);
                result.bind_mut().set_origin_dataset(self.to_gd());
                return Some(result);
            }
        }
        None
    }

    /// Returns a virtual GeoFeatureLayer containing the results of the given SQL
    /// query. It is recommended to check the validity of the returned
    /// object with GeoFeatureLayer::is_valid().
    #[func]
    pub fn get_sql_feature_layer(&self, query: GString) -> Option<Gd<GeoFeatureLayer>> {
        let query_string = query.to_string();
        if let Some(dataset) = self.dataset.as_ref() {
            if !dataset.is_valid() {
                return None;
            }
            if let Some(sql_layer) = dataset.get_sql_layer(&query_string) {
                let mut result = GeoFeatureLayer::new_gd();
                result.bind_mut().set_name(query);
                result.bind_mut().set_native_layer(sql_layer);
                result.bind_mut().set_origin_dataset(self.to_gd());
                return Some(result);
            }
        }
        None
    }

    /// Load a dataset file such as a Geopackage or a Shapefile into this
    /// object.
    #[func]
    pub fn load_from_file(&mut self, file_path: GString, write_access: bool) {
        self.write_access = write_access;
        let path = Path::new(&file_path.to_string()).to_path_buf();
        self.name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .into();
        match VectorExtractor::open_dataset(&path, write_access) {
            Ok(dataset) => self.dataset = Some(dataset),
            Err(err) => godot_error!("Could not load dataset. {err}"),
        }
    }

    /// Set the GDALDataset object directly.
    /// Not exposed to Godot since Godot doesn't know about GDALDatasets - this
    /// is only for internal use.
    #[allow(
        dead_code,
        reason = "TODO: needs to be revised, since it really isn't used at all"
    )]
    pub fn set_native_dataset(&mut self, new_dataset: NativeDataset) {
        self.dataset = Some(new_dataset);
    }
}
