use std::{cell::RefCell, collections::HashMap, rc::Rc};

use godot::{
    classes::RefCounted,
    obj::{Bounds, NewGd, bounds::DeclUser},
    prelude::*,
};

use crate::{
    geo_features::{GeoFeature, GeoGeneralFeature, GeoLine, GeoPoint, GeoPolygon},
    global::{Coords2D, ExtentData},
    vector_extractor::{
        DynFeatureHolder, Feature, GeometryType, LineFeature, NativeLayer, PointFeature,
        PolygonFeature, UnknownFeature,
    },
};

use super::geo_dataset::GeoDataset;

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoFeatureLayer {
    base: Base<RefCounted>,
    layer: Option<Rc<RefCell<NativeLayer>>>,
    origin_dataset: Gd<GeoDataset>,
    extent_data: ExtentData,
    #[var(pub)]
    name: GString,
    feature_cache: HashMap<DynFeatureHolder, DynGd<RefCounted, dyn GeoFeature<dyn Feature>>>,
}

#[godot_api]
impl GeoFeatureLayer {
    /// Emitted when a new feature has been added to the layer via `create_feature`.
    #[signal]
    fn feature_added(new_feature: Gd<RefCounted>);

    /// Emitted when a feature has been removed from the layer via `remove_feature`.
    #[signal]
    fn feature_removed(removed_feature: Gd<RefCounted>);

    /// Returns true if the layer could successfully be loaded.
    #[func]
    pub fn is_valid(&self) -> bool {
        self.layer
            .as_ref()
            .is_some_and(|layer| layer.borrow().is_valid())
    }

    /// Returns information about this file:
    /// `path`: the path to the dataset
    /// `name`: the name of the layer within the dataset
    #[func]
    pub fn get_file_info(&self) -> Dictionary<GString, GString> {
        let mut result = Dictionary::new();
        result.set("name", self.name.clone().to_godot());
        if let Some(dataset) = self.origin_dataset.bind().get_dataset() {
            result.set("path", dataset.path.to_str().unwrap());
        }
        result
    }

    /// Returns the dataset which this layer was opened from. Never `null` because feature layers must have an underlying dataset.
    #[func]
    pub fn get_dataset(&self) -> Option<Gd<GeoDataset>> {
        if !self.is_valid() || !self.origin_dataset.bind().is_valid() {
            godot_error!("GeoFeatureLayer or the data from its origin_dataset is invalid");
            return None;
        }
        Some(self.origin_dataset.clone())
    }

    /// Returns the spatial reference of the underlying dataset.
    #[func]
    pub fn get_epsg_code(&self) -> i32 {
        if !self.is_valid() || !self.origin_dataset.bind().is_valid() {
            godot_error!("Cannot get EPSK code of invalid GeoFeatureLayer or origin_dataset");
            return -1;
        }
        self.origin_dataset
            .bind()
            .get_dataset()
            .unwrap()
            .get_epsg_code()
            .unwrap_or_else(|| {
                godot_error!("EPSG code from origin_dataset could not be properly retrieved.");
                -1
            })
    }

    /// Returns the point in the center of the layer in projected meters.
    /// The y-component is 0.0.
    #[func]
    pub fn get_center(&self) -> Vector3 {
        Vector3::new(
            (self.extent_data.left + (self.extent_data.right - self.extent_data.left) / 2.0) as f32,
            0.0,
            (self.extent_data.top + (self.extent_data.down - self.extent_data.top) / 2.0) as f32,
        )
    }

    /// Returns the one feature that corresponds to the given ID.
    #[func]
    pub fn get_feature_by_id(
        &mut self,
        id: i64,
    ) -> Option<DynGd<RefCounted, dyn GeoFeature<dyn Feature>>> {
        let layer_clone = self.layer.clone();
        if self.is_valid()
            && let Some(layer) = layer_clone.as_ref()
            && let Some(feature) = layer
                .borrow()
                .get_feature_by_id::<UnknownFeature>(id as u64)
        {
            let raw_feature: Rc<RefCell<dyn Feature>> = Rc::new(RefCell::new(feature));
            return Some(self.get_specialized_feature(raw_feature));
        }
        None
    }

    /// Returns all features, regardless of the geometry, within this layer.
    #[func]
    pub fn get_all_features(&mut self) -> Array<DynGd<RefCounted, dyn GeoFeature<dyn Feature>>> {
        let mut result = Array::new();
        if !self.is_valid() {
            godot_error!("Can't get features in invalid GeoFeatureLayer!");
            return result;
        }

        let Some(layer) = self.layer.clone() else {
            return result;
        };
        let features = layer.borrow_mut().get_features();
        let Some(features) = features else {
            return result;
        };

        for feature in features {
            let raw_feature: Rc<RefCell<dyn Feature>> = Rc::new(RefCell::new(feature));
            let specialized = self.get_specialized_feature(raw_feature);
            result.push(&specialized);
        }
        result
    }

    /// Adds the given feature to the layer.
    /// This change has no effect on the dataset on disk unless save_override or save_new is called.
    #[func]
    pub fn create_feature(&mut self) -> Option<DynGd<RefCounted, dyn GeoFeature<dyn Feature>>> {
        if !self.is_valid() {
            godot_error!("Can't create feature in invalid GeoFeatureLayer!");
            return None;
        }

        let layer = self.layer.clone()?;
        let created = layer.borrow_mut().create_feature();
        let gdal_feature = match created {
            Ok(Some(gdal_feature)) => gdal_feature,
            Ok(None) => return None,
            Err(error) => {
                godot_error!("Could not create feature: {error}");
                return None;
            }
        };

        let feature = self.get_specialized_feature(gdal_feature);
        self.signals().feature_added().emit(&feature);

        Some(feature)
    }

    /// Removes the given feature from the layer.
    /// This change has no effect on the dataset on disk unless save_override or save_new is called.
    #[func]
    pub fn remove_feature(&mut self, mut feature: DynGd<RefCounted, dyn GeoFeature<dyn Feature>>) {
        if !self.is_valid() {
            godot_error!("Can't remove feature in invalid GeoFeatureLayer!");
            return;
        }

        // Mark the feature for deletion. It is only actually removed from the dataset once
        // the layer is saved.
        feature.dyn_bind_mut().set_deleted(true);
        self.signals().feature_removed().emit(&feature);
    }

    /// Clears all cached features.
    /// Note that this will cause newly returned features not to have shared signals and changes with previously returned features.
    #[func]
    pub fn clear_cache(&mut self) {
        if !self.is_valid() {
            godot_error!("Can't clear cache in invalid GeoFeatureLayer!");
            return;
        }

        // Drop the specialized GeoFeatures cached here as well as the raw feature cache held
        // by the native layer, so that subsequently returned features are freshly built.
        self.feature_cache.clear();
        if let Some(layer) = self.layer.clone()
            && let Err(error) = layer.borrow_mut().clear_feature_cache()
        {
            godot_error!("Could not clear the native feature cache: {error}");
        }
    }

    /// Applies all changes made to the layer to this layer, overriding the previous data
    /// permanently.
    #[func]
    pub fn save_override(&self) {
        if !self.is_valid() {
            godot_error!("Can't save invalid GeoFeatureLayer!");
            return;
        }

        let origin_dataset = self.origin_dataset.bind();
        if !origin_dataset.is_valid() {
            godot_error!("Can't save in GeoFeatureLayer with invalid origin dataset!");
            return;
        }
        if !origin_dataset.has_write_access() {
            godot_error!("Cannot override a layer whose dataset was not opened with write access!");
            return;
        }

        // The C++ implementation stages edits in an in-RAM copy of the layer and flushes them
        // back here (NativeLayer::save_override / write_feature_cache_to_ram_layer). That
        // staging subsystem has not been ported to the Rust NativeLayer yet, so there is
        // nothing to persist.
        godot_error!(
            "Persisting a GeoFeatureLayer (save_override) is not yet supported by the Rust port."
        );
    }

    /// Applies all changes made to the layer to a copy of the original layer which is created at
    /// the given path. Will be created as a Shapefile.
    #[func]
    pub fn save_new(&self, file_path: GString) {
        if !self.is_valid() {
            godot_error!("Can't save invalid GeoFeatureLayer!");
            return;
        }

        // See `save_override`: the layer-copy/save subsystem from the C++ NativeLayer
        // (save_modified_layer) has not been ported to the Rust NativeLayer yet.
        let _ = file_path;
        godot_error!(
            "Persisting a GeoFeatureLayer (save_new) is not yet supported by the Rust port."
        );
    }

    /// Returns all features, regardless of the geometry, near the given
    /// position (within the given radius).
    #[func]
    pub fn get_features_near_position(
        &mut self,
        pos_x: f64,
        pos_y: f64,
        radius: f64,
        max_features: i64,
    ) -> Array<DynGd<RefCounted, dyn GeoFeature<dyn Feature>>> {
        let mut result = Array::new();
        if !self.is_valid() {
            godot_error!("Can't get features in invalid GeoFeatureLayer!");
            return result;
        }

        let Some(layer) = self.layer.clone() else {
            return result;
        };
        let raw_features = match layer.borrow_mut().get_features_near_position(
            Coords2D(pos_x, pos_y),
            radius,
            max_features as u64,
        ) {
            Ok(raw_features) => raw_features,
            Err(error) => {
                godot_error!("Could not get features near position: {error}");
                return result;
            }
        };

        for raw_feature in raw_features {
            let raw_feature: Rc<RefCell<dyn Feature>> = Rc::new(RefCell::new(raw_feature));
            let specialized = self.get_specialized_feature(raw_feature);
            result.push(&specialized);
        }
        result
    }

    /// Returns all features which intersect with the square constructed by the given top-left and size.
    #[func]
    pub fn get_features_in_square(
        &mut self,
        top_left_x: f64,
        top_left_y: f64,
        size_meters: f64,
        max_features: i64,
    ) -> Array<DynGd<RefCounted, dyn GeoFeature<dyn Feature>>> {
        let mut result = Array::new();
        if !self.is_valid() {
            godot_error!("Can't get features in invalid GeoFeatureLayer!");
            return result;
        }

        let Some(layer) = self.layer.clone() else {
            return result;
        };
        let raw_features = match layer.borrow_mut().get_features_in_square(
            Coords2D(top_left_x, top_left_y),
            size_meters,
            max_features as u64,
        ) {
            Ok(raw_features) => raw_features,
            Err(error) => {
                godot_error!("Could not get features in square: {error}");
                return result;
            }
        };

        for raw_feature in raw_features {
            let raw_feature: Rc<RefCell<dyn Feature>> = Rc::new(RefCell::new(raw_feature));
            let specialized = self.get_specialized_feature(raw_feature);
            result.push(&specialized);
        }
        result
    }

    /// Returns an Array containing the names of all attributes in this layer and its features.
    #[func]
    pub fn get_attribute_names(&self) -> Array<GString> {
        let Some(layer) = self.layer.as_ref() else {
            return Array::new();
        };
        layer
            .borrow()
            .get_field_names()
            .iter()
            .map(|field_name| field_name.to_godot())
            .collect()
    }

    /// Returns `true` if there is an attribute with the given name in the layer and its features.
    #[func]
    pub fn has_attribute(&self, attribute_name: GString) -> bool {
        let Some(layer) = self.layer.as_ref() else {
            return false;
        };
        layer.borrow().field_exists(&attribute_name.to_string())
    }

    /// Returns all features which fulfill the given SQL-WHERE-like attribute filter, e.g. "attributename < 1000"
    /// Note that syntax errors are printed to the console by GDAL.
    #[func]
    pub fn get_features_by_attribute_filter(
        &mut self,
        filter: GString,
    ) -> Array<DynGd<RefCounted, dyn GeoFeature<dyn Feature>>> {
        let mut result = Array::new();
        if !self.is_valid() {
            godot_error!("Can't get features in invalid GeoFeatureLayer!");
            return result;
        }

        let Some(layer) = self.layer.clone() else {
            return result;
        };
        let filter = filter.to_string();
        let raw_features = layer
            .borrow_mut()
            .get_features_by_attribute_filter(Some(filter.as_str()));

        if let Some(raw_features) = raw_features {
            for raw_feature in raw_features {
                let raw_feature: Rc<RefCell<dyn Feature>> = Rc::new(RefCell::new(raw_feature));
                let specialized = self.get_specialized_feature(raw_feature);
                result.push(&specialized);
            }
        }
        result
    }

    fn new_feature_from_geo_feature_type_and_feature_type<T, U>(
        raw_feature: Rc<RefCell<dyn Feature>>,
    ) -> Variant
    where
        T: NewGd + AsDyn<dyn GeoFeature<U>> + Bounds<Declarer = DeclUser> + Inherits<RefCounted>,
        U: Feature + 'static + Clone,
    {
        let geo_feature = T::new_gd();
        let downcasted_feature = raw_feature
            .borrow()
            .as_any()
            .downcast_ref::<U>()
            .unwrap()
            .clone();
        let feature = Rc::new(RefCell::new(downcasted_feature));
        let mut dyn_geofeature = geo_feature.into_dyn::<dyn GeoFeature<U>>();
        dyn_geofeature.dyn_bind_mut().set_gdal_feature(feature);
        dyn_geofeature.upcast::<RefCounted>().to_variant()
    }

    /// Returns the (cached) specialized feature of the given raw feature
    ///
    /// If the raw_feature is not there, it is added to the cache and a new (now chached) GeoFeature is returned
    pub fn get_specialized_feature(
        &mut self,
        raw_feature: Rc<RefCell<dyn Feature>>,
    ) -> DynGd<RefCounted, dyn GeoFeature<dyn Feature>> {
        if let Some(cached) = self
            .feature_cache
            .get(&DynFeatureHolder(Rc::clone(&raw_feature)))
        {
            return cached.clone();
        }
        // Not cached -> instantiate and cache

        // Check which geometry this feature has, and cast it to the according
        // specialized class
        let geometry_type = raw_feature.borrow().geometry_type();
        let new_feature = match geometry_type {
            GeometryType::Point => Self::new_feature_from_geo_feature_type_and_feature_type::<
                GeoPoint,
                PointFeature,
            >(Rc::clone(&raw_feature)),
            GeometryType::Line => Self::new_feature_from_geo_feature_type_and_feature_type::<
                GeoLine,
                LineFeature,
            >(Rc::clone(&raw_feature)),
            GeometryType::Polygon => Self::new_feature_from_geo_feature_type_and_feature_type::<
                GeoPolygon,
                PolygonFeature,
            >(Rc::clone(&raw_feature)),
            GeometryType::MultiLine
            | GeometryType::MultiPolygon
            | GeometryType::Unknown
            | GeometryType::None => Self::new_feature_from_geo_feature_type_and_feature_type::<
                GeoGeneralFeature,
                UnknownFeature,
            >(Rc::clone(&raw_feature)),
        };
        let re_enriched: DynGd<RefCounted, dyn GeoFeature<dyn Feature>> = new_feature.to();
        self.feature_cache
            .insert(DynFeatureHolder(raw_feature), re_enriched.clone());

        re_enriched
    }

    /// Set the OGRLayer object directly.
    /// Not exposed to Godot since Godot doesn't know about GDALDatasets - this
    /// is only for internal use.
    pub fn set_native_layer(&mut self, new_layer: NativeLayer) {
        self.layer = Some(Rc::new(RefCell::new(new_layer)));
    }

    /// Sets the dataset which this layer was opened from.
    /// Not exposed to Godot since it should never construct GeoFeatureLayers by hand.
    pub fn set_origin_dataset(&mut self, dataset: Gd<GeoDataset>) {
        self.origin_dataset = dataset;
    }
}
