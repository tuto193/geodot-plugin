use std::{cell::RefCell, path::PathBuf, rc::Rc};

use gdal::{
    Dataset as GdalDataset, DatasetOptions, GdalOpenFlags, Metadata, errors::GdalError,
    vector::LayerAccess,
};
use godot::global::godot_error;

use super::NativeLayer;

/// Owns an open GDAL dataset and exposes vector- and raster-oriented queries
/// over it.
pub struct NativeDataset {
    pub gdal_dataset: Option<RefCell<GdalDataset>>,
    write_access: bool,
    pub path: PathBuf,
}

impl NativeDataset {
    /// Opens the dataset at `path`, optionally with write access.
    pub fn new(path: &PathBuf, write_access: bool) -> Result<Self, GdalError> {
        let open_flags = if write_access {
            GdalOpenFlags::GDAL_OF_UPDATE
        } else {
            GdalOpenFlags::GDAL_OF_READONLY
        };
        let dataset = GdalDataset::open_ex(
            path,
            DatasetOptions {
                open_flags,
                allowed_drivers: None,
                open_options: None,
                sibling_files: None,
            },
        )?;
        Ok(Self {
            gdal_dataset: Some(RefCell::new(dataset)),
            write_access,
            path: path.clone(),
        })
    }
    /// Return the names of all feature layers as std::strings.
    pub fn get_feature_layer_names(&self) -> Option<Vec<String>> {
        if let Some(dataset) = self.gdal_dataset.as_ref() {
            return Some(
                dataset
                    .borrow()
                    .layers()
                    .map(|layer| layer.name())
                    .collect(),
            );
        }
        None
    }
    /// Return the names of all raster layers as std::strings.
    pub fn get_raster_layer_names(&self) -> Option<Vec<String>> {
        if let Some(dataset) = self.gdal_dataset.as_ref()
            && let Some(subdataset_metadata) = dataset.borrow().metadata_domain("SUBDATASETS")
        {
            // This metadata is formated like this:
            // SUBDATASET_1_NAME=GPKG:/path/to/geopackage.gpkg:dhm
            // SUBDATASET_1_DESC=dhm - dhm
            // SUBDATASET_2_NAME=GPKG:/path/to/geopackage.gpkg:ndom
            // SUBDATASET_2_DESC=ndom - ndom
            // We want every second line (since that has the name), and only the portion after the last ':'.
            return Some(
                subdataset_metadata
                    .iter()
                    .map(|sub_dataset| {
                        let mut split_at_colons: Vec<&str> = sub_dataset.split(':').collect();
                        if let Some(last_element) = split_at_colons.pop() {
                            return last_element.to_string();
                        }
                        "".into()
                    })
                    .collect(),
            );
        }
        None
    }
    /// Return the descriptions of all raster bands as std::strings.
    pub fn get_raster_band_descriptions(&self) -> Vec<String> {
        if !self.is_valid() || self.gdal_dataset.is_none() {
            return vec![];
        }

        // Only keep bands that expose a description without erroring.
        self.gdal_dataset
            .as_ref()
            .expect("gdal_dataset presence checked above")
            .borrow()
            .rasterbands()
            .filter_map(|band| band.ok()?.description().ok())
            .collect()
    }
    /// Returns true if the dataset has a feature layer of the given name.
    // Part of the intended public/FFI API surface; not yet wired up internally.
    #[allow(dead_code)]
    pub fn has_layer(&self, name: &str) -> bool {
        self.gdal_dataset
            .as_ref()
            .is_some_and(|dataset| dataset.borrow().layer_by_name(name).is_ok())
    }
    /// Returns a layer whose features are the result of a given SQL query.
    #[allow(clippy::ptr_arg)] // Signature mirrors the C++ API and must stay `&String`.
    pub fn get_sql_layer(&self, query: &String) -> Option<NativeLayer> {
        if let Some(dataset) = self.gdal_dataset.as_ref() {
            // Copy the dataset so we can access/modify it
            let c_dataset = dataset.borrow().c_dataset();
            // SAFETY: `c_dataset` is a valid, non-null GDAL dataset handle obtained
            // from a live `GdalDataset` that this struct keeps alive. `from_c_dataset`
            // adopts the handle to build an independent wrapper over the same GDAL
            // object, which is the ownership model GDAL expects for shared datasets.
            let dataset = unsafe { GdalDataset::from_c_dataset(c_dataset) };
            let dataset = Rc::new(dataset);
            let result = NativeLayer::from_sql_result(dataset, query.clone())?;
            return Some(result);
        }
        None
    }
    /// Returns the layer from this dataset with the given name, or null if there is no layer
    /// with that name.
    #[allow(clippy::ptr_arg)] // Signature mirrors the C++ API and must stay `&String`.
    pub fn get_layer(&self, name: &String) -> Option<NativeLayer> {
        if let Some(dataset) = self.gdal_dataset.as_ref() {
            let c_dataset = dataset.borrow().c_dataset();
            // SAFETY: `c_dataset` is a valid, non-null GDAL dataset handle obtained
            // from a live `GdalDataset` owned by `self`. `from_c_dataset` wraps that
            // same underlying GDAL object so it can be consumed by `into_layer_by_name`.
            let dataset = unsafe { GdalDataset::from_c_dataset(c_dataset) };
            if let Ok(layer) = dataset.into_layer_by_name(name) {
                return Some(NativeLayer::from_owned_layer(layer));
            }
        }
        None
    }
    /// Opens a subdataset of this dataset (e.g. a layer within a GeoPackage) by name.
    #[allow(clippy::ptr_arg)] // Signature mirrors the C++ API and must stay `&String`.
    pub fn get_subdataset(&self, name: &String) -> Option<NativeDataset> {
        let new_path = format!("GPKG:{}:{}", self.path.display(), name);

        match NativeDataset::new(&PathBuf::from(&new_path), self.write_access) {
            Ok(result) => Some(result),
            Err(err) => {
                godot_error!("NativeDataset could not be created from {new_path:?}: {err}");
                None
            }
        }
    }
    /// Returns true if this wrapper holds an open dataset.
    pub fn is_valid(&self) -> bool {
        self.gdal_dataset.is_some()
    }
    /// Returns the EPSG code of the dataset's spatial reference, if it has one.
    pub fn get_epsg_code(&self) -> Option<i32> {
        if let Some(dataset) = self.gdal_dataset.as_ref() {
            match dataset.borrow().spatial_ref() {
                Ok(spatial_ref) => {
                    let auth_code = spatial_ref.auth_code().ok();
                    if let Some(auth_name) = spatial_ref.auth_name()
                        && auth_code.is_some()
                        && auth_name.eq_ignore_ascii_case("EPSG")
                    {
                        return auth_code;
                    }
                }
                Err(err) => {
                    println!("Could not retrieve Spatial Reference from dataset: {err}");
                    return None;
                }
            }
        }
        None
    }

    // pub fn get_dataset(&self) -> Option<Rc<RefCell<GdalDataset>>> {
    //     self.dataset
    //         .as_ref()
    //         .map(|ds| Rc::new(RefCell::new(ds.borrow())))
    // }

    /// Wraps a borrow of this dataset in an `Rc<RefCell<_>>`.
    // Part of the intended public/FFI API surface; not yet wired up internally.
    #[allow(dead_code)]
    pub fn clone(&self) -> Rc<RefCell<&Self>> {
        Rc::new(RefCell::new(self))
    }
}
