use std::collections::HashMap;

use super::{Feature, FromOgrFeature, GeometryType};
use gdal::vector::{Feature as OGRFeature, Geometry as OGRGeometry, OGRwkbGeometryType};

#[derive(Clone)]
pub struct PolygonFeature {
    attributes: HashMap<String, String>,
    fid: Option<u64>,
    polygon: OGRGeometry,
    geometry_type: GeometryType,
    is_deleted: bool,
}

impl Feature for PolygonFeature {
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

    // fn set_feature(&mut self, feature: &OGRFeature) {
    //     self.feature = feature;
    // }
}

impl FromOgrFeature for PolygonFeature {
    fn from_feature(feature: OGRFeature<'_>) -> Self {
        let geometry_type = GeometryType::Polygon;
        let polygon = feature
            .geometry()
            .expect("Could not extract Polygon from OGRFeature")
            .clone();
        let attributes = feature
            .fields()
            .filter_map(|(name, val)| val.and_then(|v| v.into_string().map(|s| (name, s))))
            .collect();
        Self {
            polygon,
            geometry_type,
            is_deleted: false,
            fid: feature.fid(),
            attributes,
        }
    }
}

impl PartialEq for PolygonFeature {
    fn eq(&self, other: &Self) -> bool {
        // Compare based on feature ID or other identifying characteristics
        // This is a basic implementation - adjust based on your specific needs
        self.fid == other.fid
            && self.geometry_type == other.geometry_type
            && self.is_deleted == other.is_deleted
    }
}

impl Eq for PolygonFeature {}

impl std::hash::Hash for PolygonFeature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash based on feature ID and other identifying characteristics
        // This is a basic implementation - adjust based on your specific needs
        self.fid.hash(state);
        self.geometry_type.hash(state);
        self.is_deleted.hash(state);
    }
}

impl PolygonFeature {
    pub fn get_outer_vertices(&self) -> Option<Vec<(f64, f64, f64)>> {
        if self.polygon.geometry_count() < 1 {
            return None;
        }
        let outer_ring = self.polygon.get_geometry(0).clone();
        let mut points = vec![];
        for i in 0..outer_ring.point_count() {
            let point = outer_ring.get_point(i as i32);
            points.push(point);
        }
        Some(points)
    }

    pub fn set_outer_vertices(&mut self, vertices: Vec<(f64, f64, f64)>) {
        // OGR geometries can't easily be mutated in place, so we rebuild the
        // polygon from scratch: a fresh outer ring built from `vertices`, plus
        // any pre-existing holes (interior rings) copied over unchanged.
        let mut new_polygon = OGRGeometry::empty(OGRwkbGeometryType::wkbPolygon)
            .expect("Could not create empty polygon geometry");

        let mut outer_ring = OGRGeometry::empty(OGRwkbGeometryType::wkbLinearRing)
            .expect("Could not create linear ring geometry");
        for (i, &(x, y, z)) in vertices.iter().enumerate() {
            outer_ring.set_point(i, (x, y, z));
        }

        // A valid ring must be closed: ensure the last point matches the first.
        if let (Some(&first), Some(&last)) = (vertices.first(), vertices.last())
            && first != last
        {
            outer_ring.set_point(vertices.len(), first);
        }

        new_polygon
            .add_geometry(outer_ring)
            .expect("Could not add outer ring to polygon");

        // Preserve existing holes (interior rings start at index 1).
        let total_rings = self.polygon.geometry_count();
        for interior_ring_index in 1..total_rings {
            let hole = self.polygon.get_geometry(interior_ring_index).clone();
            new_polygon
                .add_geometry(hole)
                .expect("Could not copy hole into new polygon");
        }

        self.polygon = new_polygon;
    }

    pub fn get_holes(&self) -> Vec<Vec<(f64, f64, f64)>> {
        let mut result = vec![];
        let total_rings = self.polygon.geometry_count();
        for interior_ring_index in 1..total_rings {
            let interior_ring = self.polygon.get_geometry(interior_ring_index).clone();
            let mut interior_points = vec![];
            for i in 0..interior_ring.point_count() {
                let point = interior_ring.get_point(i as i32);
                interior_points.push(point);
            }
            result.push(interior_points);
        }
        result
    }

    /// Appends a hole (interior ring) to the polygon, built from the given
    /// vertices. The ring is closed automatically if needed.
    pub fn add_hole(&mut self, vertices: Vec<(f64, f64, f64)>) {
        let mut ring = OGRGeometry::empty(OGRwkbGeometryType::wkbLinearRing)
            .expect("Could not create linear ring geometry");
        for (i, &(x, y, z)) in vertices.iter().enumerate() {
            ring.set_point(i, (x, y, z));
        }

        // A valid ring must be closed: ensure the last point matches the first.
        if let (Some(&first), Some(&last)) = (vertices.first(), vertices.last())
            && first != last
        {
            ring.set_point(vertices.len(), first);
        }

        self.polygon
            .add_geometry(ring)
            .expect("Could not add hole to polygon");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_extractor::features::test_util::with_feature;

    #[test]
    fn parses_outer_vertices() {
        with_feature(
            Some("POLYGON ((0 0, 2 0, 2 2, 0 2, 0 0))"),
            &[],
            |feature| {
                let polygon = PolygonFeature::from_feature(feature);
                let outer = polygon
                    .get_outer_vertices()
                    .expect("should have outer ring");
                // The ring includes the repeated closing vertex.
                assert_eq!(outer.first().copied(), Some((0.0, 0.0, 0.0)));
                assert_eq!(outer[2], (2.0, 2.0, 0.0));
            },
        );
    }

    #[test]
    fn set_outer_vertices_round_trips() {
        with_feature(Some("POLYGON ((0 0, 1 0, 1 1, 0 0))"), &[], |feature| {
            let mut polygon = PolygonFeature::from_feature(feature);
            let new_vertices = vec![
                (10.0, 10.0, 0.0),
                (20.0, 10.0, 0.0),
                (20.0, 20.0, 0.0),
                (10.0, 10.0, 0.0),
            ];
            polygon.set_outer_vertices(new_vertices.clone());

            let read_back = polygon
                .get_outer_vertices()
                .expect("should have outer ring");
            assert_eq!(read_back[0], (10.0, 10.0, 0.0));
            assert_eq!(read_back[1], (20.0, 10.0, 0.0));
            assert_eq!(read_back[2], (20.0, 20.0, 0.0));
        });
    }

    #[test]
    fn set_outer_vertices_auto_closes_ring() {
        with_feature(Some("POLYGON ((0 0, 1 0, 1 1, 0 0))"), &[], |feature| {
            let mut polygon = PolygonFeature::from_feature(feature);
            // Intentionally leave the ring open (last != first).
            polygon.set_outer_vertices(vec![(0.0, 0.0, 0.0), (5.0, 0.0, 0.0), (5.0, 5.0, 0.0)]);

            let read_back = polygon
                .get_outer_vertices()
                .expect("should have outer ring");
            // GDAL closes the ring, so the first and last vertices must match.
            assert_eq!(read_back.first(), read_back.last());
            assert_eq!(read_back.len(), 4);
        });
    }

    #[test]
    fn add_hole_appends_interior_ring() {
        with_feature(
            Some("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))"),
            &[],
            |feature| {
                let mut polygon = PolygonFeature::from_feature(feature);
                assert_eq!(polygon.get_holes().len(), 0);

                polygon.add_hole(vec![
                    (2.0, 2.0, 0.0),
                    (4.0, 2.0, 0.0),
                    (4.0, 4.0, 0.0),
                    (2.0, 2.0, 0.0),
                ]);

                let holes = polygon.get_holes();
                assert_eq!(holes.len(), 1);
                assert_eq!(holes[0][0], (2.0, 2.0, 0.0));

                // The outer ring must be untouched by adding a hole.
                let outer = polygon
                    .get_outer_vertices()
                    .expect("should have outer ring");
                assert_eq!(outer[0], (0.0, 0.0, 0.0));
            },
        );
    }

    #[test]
    fn get_holes_reads_existing_interior_rings() {
        with_feature(
            Some("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0), (2 2, 4 2, 4 4, 2 2))"),
            &[],
            |feature| {
                let polygon = PolygonFeature::from_feature(feature);
                let holes = polygon.get_holes();
                assert_eq!(holes.len(), 1);
                assert_eq!(holes[0][0], (2.0, 2.0, 0.0));
            },
        );
    }

    #[test]
    fn geometry_type_is_polygon() {
        with_feature(Some("POLYGON ((0 0, 1 0, 1 1, 0 0))"), &[], |feature| {
            let polygon = PolygonFeature::from_feature(feature);
            assert_eq!(polygon.geometry_type(), GeometryType::Polygon);
        });
    }
}
