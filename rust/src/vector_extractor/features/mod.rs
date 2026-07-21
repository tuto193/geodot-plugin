mod feature;
mod line_feature;
mod point_feature;
mod polygon_feature;
mod unknown_feature;

pub use feature::{DynFeatureHolder, Feature, FromOgrFeature, GeometryType};
pub use line_feature::LineFeature;
pub use point_feature::PointFeature;
pub use polygon_feature::PolygonFeature;
pub use unknown_feature::UnknownFeature;

#[cfg(test)]
pub(crate) mod test_util {
    use gdal::vector::{
        Feature as OGRFeature, Geometry as OGRGeometry, LayerAccess, LayerOptions, OGRFieldType,
        OGRwkbGeometryType,
    };
    use gdal::{Dataset, DriverManager};

    /// Creates an in-memory GDAL dataset with a single layer whose fields match
    /// the provided `(name, value)` string attributes.
    ///
    /// GDAL requires the dataset and layer to outlive any `OGRFeature` borrowed
    /// from them, so this returns the owning `Dataset` alongside the built
    /// feature via a callback to keep the lifetimes sound.
    pub fn with_feature<R>(
        wkt: Option<&str>,
        fields: &[(&str, &str)],
        callback: impl FnOnce(OGRFeature<'_>) -> R,
    ) -> R {
        // The ephemeral in-memory driver keeps everything in RAM and needs no
        // filesystem access.
        let driver = DriverManager::get_driver_by_name("MEM")
            .expect("the in-memory 'MEM' driver should be available");
        let mut dataset: Dataset = driver
            .create_vector_only("in_memory")
            .expect("could not create in-memory dataset");
        // When no geometry is requested, create a geometry-less layer so the
        // resulting feature genuinely reports no geometry.
        let geometry_type = if wkt.is_some() {
            OGRwkbGeometryType::wkbUnknown
        } else {
            OGRwkbGeometryType::wkbNone
        };
        let layer = dataset
            .create_layer(LayerOptions {
                name: "layer",
                ty: geometry_type,
                ..Default::default()
            })
            .expect("could not create layer");

        let field_defs: Vec<(&str, OGRFieldType::Type)> = fields
            .iter()
            .map(|(name, _)| (*name, OGRFieldType::OFTString))
            .collect();
        layer
            .create_defn_fields(&field_defs)
            .expect("could not define fields");

        let mut feature =
            OGRFeature::new(layer.defn()).expect("could not create feature from definition");
        for (index, (_, value)) in fields.iter().enumerate() {
            feature
                .set_field_string(index, value)
                .expect("could not set field value");
        }
        if let Some(wkt) = wkt {
            let geometry = OGRGeometry::from_wkt(wkt).expect("invalid WKT geometry");
            feature
                .set_geometry(geometry)
                .expect("could not set geometry");
        }

        callback(feature)
    }
}
