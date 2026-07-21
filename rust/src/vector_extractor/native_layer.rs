use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    global::{Coords2D, ExtentData},
    vector_extractor::{
        LineFeature, PointFeature, PolygonFeature, UnknownFeature,
        features::{Feature, FromOgrFeature},
    },
};

use gdal::{
    Dataset,
    errors::GdalError,
    vector::{
        Feature as OGRFeature, Geometry as OGRGeometry, LayerAccess, OGRFieldType,
        OGRwkbGeometryType, OwnedLayer,
    },
};

// use super::unknown_feature::UnknownFeature;

/// Backing storage for a [`NativeLayer`].
///
/// A layer is either an owned OGR layer, or the (re-runnable) result of an SQL
/// query against a dataset.
pub enum NativeLayerInner {
    Owned(OwnedLayer),
    SqlResult { dataset: Rc<Dataset>, query: String },
}

/// A vector layer wrapper exposing feature queries and (for owned layers) field
/// editing on top of GDAL/OGR.
pub struct NativeLayer {
    inner: Option<NativeLayerInner>,
    feature_cache: HashMap<u64, Vec<String>>, // Simplified cache
    disk_feature_count: u64,
    ram_feature_count: u64,
}

impl NativeLayer {
    /// Returns a reference to the backing inner layer.
    ///
    /// Callers must ensure the layer is valid (see [`is_valid`](Self::is_valid))
    /// before calling; this is upheld everywhere it is used.
    fn inner(&self) -> &NativeLayerInner {
        self.inner
            .as_ref()
            .expect("NativeLayer inner accessed while invalid")
    }

    /// Returns a mutable reference to the backing inner layer.
    ///
    /// Callers must ensure the layer is valid before calling.
    fn inner_mut(&mut self) -> &mut NativeLayerInner {
        self.inner
            .as_mut()
            .expect("NativeLayer inner accessed while invalid")
    }

    /// Wraps an owned OGR layer.
    pub fn from_owned_layer(owned_layer: OwnedLayer) -> Self {
        let disk_feature_count = owned_layer.feature_count();
        let inner = NativeLayerInner::Owned(owned_layer);
        Self {
            inner: Some(inner),
            disk_feature_count,
            feature_cache: HashMap::new(),
            ram_feature_count: 0,
        }
    }

    /// Builds a layer from running `query` against `dataset`, or `None` if the
    /// query produced no result set.
    pub fn from_sql_result(dataset: Rc<Dataset>, query: String) -> Option<Self> {
        let cloned = Rc::clone(&dataset);
        let rs = cloned
            .execute_sql(query.clone(), None, gdal::vector::sql::Dialect::DEFAULT)
            .ok()??;
        let disk_feature_count = rs.feature_count();
        let inner = NativeLayerInner::SqlResult { dataset, query };
        Some(Self {
            inner: Some(inner),
            feature_cache: HashMap::new(),
            disk_feature_count,
            ram_feature_count: 0,
        })
    }

    /// Returns true if this wrapper holds a layer.
    pub fn is_valid(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns the spatial extent of the layer, if available.
    // Part of the intended public/FFI API surface; not yet wired up internally.
    #[allow(dead_code)]
    pub fn get_extent(&self) -> Option<ExtentData> {
        if !self.is_valid() {
            return None;
        }
        match self.inner.as_ref()? {
            NativeLayerInner::Owned(layer) => {
                layer.get_extent().ok().map(ExtentData::from_envelope)
            }
            NativeLayerInner::SqlResult { dataset, query } => {
                let rs = dataset
                    .execute_sql(query, None, gdal::vector::sql::Dialect::DEFAULT)
                    .ok()??;
                rs.get_extent().ok().map(ExtentData::from_envelope)
            }
        }
    }

    /// Clears the internal feature cache.
    pub fn clear_feature_cache(&mut self) -> Result<(), GdalError> {
        self.feature_cache.clear();
        Ok(())
    }

    /// Returns the feature with the given id, wrapped as `T`, if present.
    pub fn get_feature_by_id<T>(&self, id: u64) -> Option<T>
    where
        T: Feature + FromOgrFeature,
    {
        if self.is_valid() {
            match self.inner.as_ref()? {
                NativeLayerInner::Owned(layer) => {
                    let to_wrap = layer.feature(id)?;

                    return Some(T::from_feature(to_wrap));
                }
                NativeLayerInner::SqlResult { dataset, query } => {
                    let layer = dataset
                        .execute_sql(query, None, gdal::vector::sql::Dialect::DEFAULT)
                        .ok()??;
                    let to_wrap = layer.feature(id)?;
                    return Some(T::from_feature(to_wrap));
                }
            }
        }
        None
    }

    /// Returns all features matching an optional OGR attribute filter.
    pub fn get_features_by_attribute_filter(
        &mut self,
        filter: Option<&str>,
    ) -> Option<Vec<UnknownFeature>> {
        let mut result = vec![];
        if self.is_valid() {
            match self.inner.as_mut()? {
                NativeLayerInner::Owned(layer) => {
                    layer.reset_feature_reading();
                    if filter.is_none_or(|f| layer.set_attribute_filter(f).is_ok()) {
                        for f in layer.features() {
                            result.push(UnknownFeature::from_feature(f));
                        }
                    }
                }
                NativeLayerInner::SqlResult { dataset, query } => {
                    let mut layer = dataset
                        .execute_sql(query, None, gdal::vector::sql::Dialect::DEFAULT)
                        .ok()??;
                    layer.reset_feature_reading();
                    if filter.is_none_or(|f| layer.set_attribute_filter(f).is_ok()) {
                        for f in layer.features() {
                            result.push(UnknownFeature::from_feature(f));
                        }
                    }
                }
            };
        }
        Some(result)
    }

    /// Returns all features in the layer.
    pub fn get_features(&mut self) -> Option<Vec<UnknownFeature>> {
        self.get_features_by_attribute_filter(None)
    }

    /// Returns up to `max_amount` features within `radius` of `position`.
    pub fn get_features_near_position(
        &mut self,
        position: Coords2D,
        radius: f64,
        max_amount: u64,
    ) -> Result<Vec<UnknownFeature>, GdalError> {
        let point_wkt_string = format!("POINT ({position})");
        let circle = OGRGeometry::from_wkt(&point_wkt_string)?;
        let quad_segments = 30;
        let circle_buffer = circle.buffer(radius, quad_segments)?;
        self.get_features_inside_geometry(&circle_buffer, max_amount)
    }

    /// Returns up to `max_amount` features inside a square starting at `top_left`.
    pub fn get_features_in_square(
        &mut self,
        top_left: Coords2D,
        size_meters: f64,
        max_amount: u64,
    ) -> Result<Vec<UnknownFeature>, GdalError> {
        let bottom_left = Coords2D(top_left.0, top_left.1 - size_meters);
        let bottom_right = Coords2D(top_left.0 + size_meters, top_left.1 - size_meters);
        let top_right = Coords2D(top_left.0 + size_meters, top_left.1);

        let square_polygon_string = format!(
            "POLYGON (({top_left}, {bottom_left}, {bottom_right}, {top_right}, {top_left}))"
        );
        let square_outline = OGRGeometry::from_wkt(&square_polygon_string)?;
        self.get_features_inside_geometry(&square_outline, max_amount)
    }

    fn get_features_inside_geometry(
        &mut self,
        geometry: &OGRGeometry,
        max_amount: u64,
    ) -> Result<Vec<UnknownFeature>, GdalError> {
        let mut result = vec![];
        match self.inner_mut() {
            NativeLayerInner::Owned(layer) => {
                layer.set_spatial_filter(geometry);
                layer.reset_feature_reading();
                for feature in layer.features().take(max_amount as usize) {
                    result.push(UnknownFeature::from_feature(feature));
                }
            }
            NativeLayerInner::SqlResult { dataset, query } => {
                if let Some(mut layer) =
                    dataset.execute_sql(query, None, gdal::vector::sql::Dialect::DEFAULT)?
                {
                    layer.set_spatial_filter(geometry);
                    layer.reset_feature_reading();
                    for feature in layer.features().take(max_amount as usize) {
                        result.push(UnknownFeature::from_feature(feature));
                    }
                }
            }
        }
        Ok(result)
    }

    /// Adds a new string field with the given name to the layer.
    ///
    /// Note: unlike the C++ version, which stages the change on an in-RAM copy of the
    /// layer, this operates directly on the underlying layer, so the dataset must have
    /// been opened with write access. SQL result layers cannot be modified.
    // Part of the intended public/FFI API surface; not yet wired up internally.
    #[allow(dead_code)]
    pub fn add_field(&mut self, name: String) -> Result<(), GdalError> {
        if !self.is_valid() {
            return Err(GdalError::BadArgument("No layer available".to_string()));
        }

        // According to GDAL, no feature objects may exist while altering field
        // definitions, so clear the cache first.
        self.feature_cache.clear();

        match self.inner() {
            NativeLayerInner::Owned(layer) => {
                layer.create_defn_fields(&[(name.as_str(), OGRFieldType::OFTString)])
            }
            NativeLayerInner::SqlResult { .. } => Err(GdalError::BadArgument(
                "Fields of a SQL result layer cannot be modified".to_string(),
            )),
        }
    }

    /// Removes the field with the given name from the layer.
    ///
    /// As with [`add_field`](Self::add_field), this operates directly on the underlying
    /// layer and requires write access. SQL result layers cannot be modified.
    // Part of the intended public/FFI API surface; not yet wired up internally.
    #[allow(dead_code)]
    pub fn remove_field(&mut self, name: String) -> Result<(), GdalError> {
        if !self.is_valid() {
            return Err(GdalError::BadArgument("No layer available".to_string()));
        }

        // According to GDAL, no feature objects may exist while altering field
        // definitions, so clear the cache first.
        self.feature_cache.clear();

        match self.inner() {
            NativeLayerInner::Owned(layer) => {
                let index = layer.defn().field_index(&name)?;
                // The gdal crate has no safe wrapper for deleting a field, so call
                // into gdal-sys directly.
                // SAFETY: `layer.c_layer()` returns the valid, non-null OGR layer
                // handle owned by this `OwnedLayer`, and `index` was just obtained
                // from that same layer's definition, so it is in range. The call
                // mutates the layer in place and returns an OGRErr we check below.
                let error = unsafe { gdal_sys::OGR_L_DeleteField(layer.c_layer(), index as i32) };
                if error != gdal_sys::OGRErr::OGRERR_NONE {
                    return Err(GdalError::OgrError {
                        err: error,
                        method_name: "OGR_L_DeleteField",
                    });
                }
                Ok(())
            }
            NativeLayerInner::SqlResult { .. } => Err(GdalError::BadArgument(
                "Fields of a SQL result layer cannot be modified".to_string(),
            )),
        }
    }

    /// Returns whether a field with the given name exists on this layer.
    pub fn field_exists(&self, name: &str) -> bool {
        if !self.is_valid() {
            return false;
        }
        match self.inner() {
            NativeLayerInner::Owned(layer) => layer.defn().field_index(name).is_ok(),
            NativeLayerInner::SqlResult { dataset, query } => {
                match dataset.execute_sql(query, None, gdal::vector::sql::Dialect::DEFAULT) {
                    Ok(Some(layer)) => layer.defn().field_index(name).is_ok(),
                    _ => false,
                }
            }
        }
    }

    /// Returns the names of all fields defined on this layer.
    pub fn get_field_names(&self) -> Vec<String> {
        if !self.is_valid() {
            return vec![];
        }
        match self.inner() {
            NativeLayerInner::Owned(layer) => {
                layer.defn().fields().map(|field| field.name()).collect()
            }
            NativeLayerInner::SqlResult { dataset, query } => {
                match dataset.execute_sql(query, None, gdal::vector::sql::Dialect::DEFAULT) {
                    Ok(Some(layer)) => layer.defn().fields().map(|field| field.name()).collect(),
                    _ => vec![],
                }
            }
        }
    }

    /// Creates a new feature whose geometry type matches the layer's geometry type.
    ///
    /// The feature is created purely in memory and is *not* written to the underlying
    /// dataset, mirroring the C++ behaviour of only operating on an in-RAM copy so that
    /// the source layer can remain read-only. Persisting created features requires a
    /// dedicated save step (the C++ `save_override` / `save_modified_layer`), which has
    /// not been ported yet.
    pub fn create_feature(&mut self) -> Result<Option<Rc<RefCell<dyn Feature>>>, GdalError> {
        if !self.is_valid() {
            return Ok(None);
        }

        // Start searching for a free ID at the highest known ID plus the number of
        // features already created in memory.
        let base_id = self.disk_feature_count + self.ram_feature_count;

        let (feature, skipped) = match self.inner() {
            NativeLayerInner::Owned(layer) => {
                let defn = layer.defn();
                let geometry_type = defn.geometry_type();

                // Build a new in-memory feature carrying an empty geometry of the
                // layer's type.
                let mut new_feature = OGRFeature::new(defn)?;
                new_feature.set_geometry(OGRGeometry::empty(geometry_type)?)?;

                // Ensure the generated ID is unused, skipping over any gaps that are
                // already occupied in the original data.
                let mut id = base_id;
                let mut skipped: u64 = 0;
                while layer.feature(id).is_some() {
                    skipped += 1;
                    id += 1;
                }

                // The gdal crate has no safe wrapper for setting the FID.
                // SAFETY: `new_feature.c_feature()` is the valid, non-null OGR
                // feature handle owned by the `OGRFeature` created just above, and
                // `id` is a plain integer FID. Setting the FID only mutates that
                // in-memory feature and transfers no ownership.
                unsafe { gdal_sys::OGR_F_SetFID(new_feature.c_feature(), id as i64) };

                // Instantiate the specialized feature type corresponding to the
                // layer's geometry, matching the C++ `create_feature` dispatch.
                let feature: Rc<RefCell<dyn Feature>> =
                    if geometry_type == OGRwkbGeometryType::wkbPoint {
                        Rc::new(RefCell::new(PointFeature::from_feature(new_feature)))
                    } else if geometry_type == OGRwkbGeometryType::wkbLineString {
                        Rc::new(RefCell::new(LineFeature::from_feature(new_feature)))
                    } else if geometry_type == OGRwkbGeometryType::wkbPolygon {
                        Rc::new(RefCell::new(PolygonFeature::from_feature(new_feature)))
                    } else {
                        Rc::new(RefCell::new(UnknownFeature::from_feature(new_feature)))
                    };

                (feature, skipped)
            }
            NativeLayerInner::SqlResult { .. } => {
                return Err(GdalError::BadArgument(
                    "Features cannot be created on a SQL result layer".to_string(),
                ));
            }
        };

        // Keep the ID counters in sync so subsequent calls produce unique IDs.
        self.disk_feature_count += skipped;
        self.ram_feature_count += 1;

        Ok(Some(feature))
    }
}
