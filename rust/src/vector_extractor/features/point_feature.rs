use std::collections::HashMap;

use super::{Feature, FromOgrFeature, GeometryType};

use gdal::vector::{Feature as OGRFeature, Geometry as OGRGeometry};

#[derive(Clone)]
pub struct PointFeature {
    vector: (f64, f64, f64),
    attributes: HashMap<String, String>,
    fid: Option<u64>,
    point: OGRGeometry,
    geometry_type: GeometryType,
    is_deleted: bool,
}

impl Feature for PointFeature {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_deleted(&mut self, deleted: bool) {
        self.is_deleted = deleted;
    }

    fn is_deleted(&self) -> bool {
        self.is_deleted
    }
    fn geometry_type(&self) -> super::feature::GeometryType {
        self.geometry_type
    }

    fn get_id(&self) -> Option<u64> {
        self.fid
    }

    fn get_attributes(&self) -> HashMap<String, String> {
        self.attributes.clone()
    }

    fn get_attribute(&self, field_name: String) -> Option<String> {
        self.attributes.get(&field_name).cloned()
    }

    /// Updates the in-memory attribute. Changes are not written back to the
    /// source dataset until a save operation is performed.
    fn set_attribute(&mut self, name: String, value: String) {
        self.attributes.insert(name, value);
    }
}

impl FromOgrFeature for PointFeature {
    fn from_feature(feature: OGRFeature<'_>) -> Self {
        let point = feature
            .geometry()
            .expect("Feature should have a geometry (point) set.")
            .clone();
        let vector = point.get_point(0);
        let geometry_type = GeometryType::Point;
        let attributes = feature
            .fields()
            .filter_map(|(name, val)| val.and_then(|v| v.into_string().map(|s| (name, s))))
            .collect();
        Self {
            point,
            vector,
            geometry_type,
            fid: feature.fid(),
            attributes,
            is_deleted: false,
        }
    }
}

impl PartialEq for PointFeature {
    fn eq(&self, other: &Self) -> bool {
        self.vector == other.vector
            && self.geometry_type == other.geometry_type
            && self.is_deleted == other.is_deleted
    }
}

impl Eq for PointFeature {}

impl std::hash::Hash for PointFeature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.vector.0.to_bits().hash(state);
        self.vector.1.to_bits().hash(state);
        self.vector.2.to_bits().hash(state);
        self.geometry_type.hash(state);
        self.is_deleted.hash(state);
    }
}

impl PointFeature {
    pub fn set_vector(&mut self, vector: (f64, f64, f64)) {
        self.vector = vector;
        self.point.set_point(0, vector);
    }
    pub fn get_vector(&self) -> (f64, f64, f64) {
        self.vector
    }

    // Coordinate accessors are part of the intended public/FFI API surface.
    #[allow(dead_code)]
    pub fn get_x(&self) -> f64 {
        self.vector.0
    }
    #[allow(dead_code)]
    pub fn get_y(&self) -> f64 {
        self.vector.1
    }
    #[allow(dead_code)]
    pub fn get_z(&self) -> f64 {
        self.vector.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_extractor::features::test_util::with_feature;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(feature: &PointFeature) -> u64 {
        let mut hasher = DefaultHasher::new();
        feature.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn parses_geometry_and_coordinates() {
        with_feature(Some("POINT Z (1 2 3)"), &[], |feature| {
            let point = PointFeature::from_feature(feature);
            assert_eq!(point.get_vector(), (1.0, 2.0, 3.0));
            assert_eq!(point.get_x(), 1.0);
            assert_eq!(point.get_y(), 2.0);
            assert_eq!(point.get_z(), 3.0);
            assert_eq!(point.geometry_type(), GeometryType::Point);
        });
    }

    #[test]
    fn extracts_string_attributes() {
        with_feature(Some("POINT (0 0)"), &[("name", "vienna")], |feature| {
            let point = PointFeature::from_feature(feature);
            assert_eq!(
                point.get_attribute("name".to_string()),
                Some("vienna".to_string())
            );
            assert_eq!(point.get_attribute("missing".to_string()), None);
            assert_eq!(point.get_attributes().len(), 1);
        });
    }

    #[test]
    fn set_vector_updates_coordinates() {
        with_feature(Some("POINT (0 0)"), &[], |feature| {
            let mut point = PointFeature::from_feature(feature);
            point.set_vector((4.0, 5.0, 6.0));
            assert_eq!(point.get_vector(), (4.0, 5.0, 6.0));
        });
    }

    #[test]
    fn set_attribute_inserts_and_overwrites() {
        with_feature(Some("POINT (0 0)"), &[("name", "old")], |feature| {
            let mut point = PointFeature::from_feature(feature);
            point.set_attribute("name".to_string(), "new".to_string());
            assert_eq!(
                point.get_attribute("name".to_string()),
                Some("new".to_string())
            );
        });
    }

    #[test]
    fn deletion_flag_round_trips() {
        with_feature(Some("POINT (0 0)"), &[], |feature| {
            let mut point = PointFeature::from_feature(feature);
            assert!(!point.is_deleted());
            point.set_deleted(true);
            assert!(point.is_deleted());
        });
    }

    #[test]
    fn equality_and_hashing_are_consistent() {
        with_feature(Some("POINT Z (1 2 3)"), &[], |feature| {
            let a = PointFeature::from_feature(feature);
            let mut b = a.clone();
            assert!(a == b);
            assert_eq!(hash_of(&a), hash_of(&b));

            b.set_vector((9.0, 9.0, 9.0));
            assert!(a != b);
        });
    }
}
