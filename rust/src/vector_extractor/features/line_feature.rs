use std::collections::HashMap;

use super::{Feature, FromOgrFeature, GeometryType};

use gdal::vector::{Feature as OGRFeature, Geometry as OGRGeometry};

#[derive(Clone)]
pub struct LineFeature {
    geometry_type: GeometryType,
    // feature: OGRFeature<'static>,
    attributes: HashMap<String, String>,
    fid: Option<u64>,
    point_count: usize,
    line: OGRGeometry,
    points: Vec<(f64, f64, f64)>,
    is_deleted: bool,
}

impl LineFeature {
    pub fn get_line_point(&self, index: usize) -> (f64, f64, f64) {
        self.points[index]
    }

    // Coordinate accessors are part of the intended public/FFI API surface.
    #[allow(dead_code)]
    pub fn get_line_point_x(&self, index: usize) -> f64 {
        self.get_line_point(index).0
    }
    #[allow(dead_code)]
    pub fn get_line_point_y(&self, index: usize) -> f64 {
        self.get_line_point(index).1
    }
    #[allow(dead_code)]
    pub fn get_line_point_z(&self, index: usize) -> f64 {
        self.get_line_point(index).2
    }

    pub fn get_point_count(&self) -> usize {
        self.point_count
    }

    pub fn set_point_count(&mut self, new_count: usize) {
        self.point_count = new_count;
    }

    pub fn set_line_point(&mut self, index: usize, point: (f64, f64, f64)) {
        self.line.set_point(index, point);
    }
}

impl std::hash::Hash for LineFeature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the geometry type
        self.geometry_type.hash(state);

        // Hash the point count
        self.point_count.hash(state);

        // Hash the points (converting f64 to bits for deterministic hashing)
        for (x, y, z) in &self.points {
            x.to_bits().hash(state);
            y.to_bits().hash(state);
            z.to_bits().hash(state);
        }

        // Hash the deleted status
        self.is_deleted.hash(state);
    }
}

impl PartialEq for LineFeature {
    fn eq(&self, other: &Self) -> bool {
        self.geometry_type == other.geometry_type
            && self.point_count == other.point_count
            && self.points == other.points
            && self.is_deleted == other.is_deleted
    }
}

impl Eq for LineFeature {}

impl Feature for LineFeature {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_deleted(&mut self, deleted: bool) {
        self.is_deleted = deleted;
    }
    fn is_deleted(&self) -> bool {
        self.is_deleted
    }
    fn geometry_type(&self) -> GeometryType {
        self.geometry_type
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

    fn get_id(&self) -> Option<u64> {
        self.fid
    }
}

impl FromOgrFeature for LineFeature {
    fn from_feature(feature: OGRFeature<'_>) -> Self {
        let mut points = vec![];
        let line = feature
            .geometry()
            .expect("Feature (line) must contain geometry.")
            .clone();
        line.get_points(&mut points);
        let point_count = line.point_count();
        let attributes = feature
            .fields()
            .filter_map(|(name, val)| val.and_then(|v| v.into_string().map(|s| (name, s))))
            .collect();
        Self {
            fid: feature.fid(),
            line,
            point_count,
            geometry_type: GeometryType::Line,
            points,
            is_deleted: false,
            attributes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_extractor::features::test_util::with_feature;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(feature: &LineFeature) -> u64 {
        let mut hasher = DefaultHasher::new();
        feature.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn parses_line_points() {
        with_feature(Some("LINESTRING (0 0, 1 1, 2 4)"), &[], |feature| {
            let line = LineFeature::from_feature(feature);
            assert_eq!(line.get_point_count(), 3);
            assert_eq!(line.get_line_point(0), (0.0, 0.0, 0.0));
            assert_eq!(line.get_line_point(2), (2.0, 4.0, 0.0));
            assert_eq!(line.get_line_point_x(1), 1.0);
            assert_eq!(line.get_line_point_y(2), 4.0);
            assert_eq!(line.get_line_point_z(0), 0.0);
            assert_eq!(line.geometry_type(), GeometryType::Line);
        });
    }

    #[test]
    fn set_point_count_updates_count() {
        with_feature(Some("LINESTRING (0 0, 1 1)"), &[], |feature| {
            let mut line = LineFeature::from_feature(feature);
            assert_eq!(line.get_point_count(), 2);
            line.set_point_count(5);
            assert_eq!(line.get_point_count(), 5);
        });
    }

    #[test]
    fn extracts_attributes() {
        with_feature(
            Some("LINESTRING (0 0, 1 1)"),
            &[("road", "A1")],
            |feature| {
                let line = LineFeature::from_feature(feature);
                assert_eq!(
                    line.get_attribute("road".to_string()),
                    Some("A1".to_string())
                );
            },
        );
    }

    #[test]
    fn deletion_flag_round_trips() {
        with_feature(Some("LINESTRING (0 0, 1 1)"), &[], |feature| {
            let mut line = LineFeature::from_feature(feature);
            assert!(!line.is_deleted());
            line.set_deleted(true);
            assert!(line.is_deleted());
        });
    }

    #[test]
    fn equality_and_hashing_are_consistent() {
        with_feature(Some("LINESTRING (0 0, 1 1, 2 2)"), &[], |feature| {
            let a = LineFeature::from_feature(feature);
            let b = a.clone();
            assert!(a == b);
            assert_eq!(hash_of(&a), hash_of(&b));

            let mut c = a.clone();
            c.set_point_count(99);
            assert!(a != c);
        });
    }
}
