use gdal::spatial_ref::{CoordTransform, SpatialRef};

/// Reprojects coordinates between two spatial reference systems.
pub struct CoordinateTransform {
    transformation: CoordTransform,
    // The source/target references are retained to keep the SRS definitions
    // that back `transformation` alive for the lifetime of this transform.
    #[allow(dead_code)]
    source_reference: SpatialRef,
    #[allow(dead_code)]
    target_reference: SpatialRef,
}

impl CoordinateTransform {
    /// Builds a transform from the `from` EPSG code to the `to` EPSG code.
    pub fn new(from: u32, to: u32) -> Self {
        let source = SpatialRef::from_epsg(from)
            .unwrap_or_else(|_| panic!("Source EPSG code '{from}' not valid"));
        let target = SpatialRef::from_epsg(to)
            .unwrap_or_else(|_| panic!("Target EPSG code '{to}' not valid"));
        let transformation = CoordTransform::new(&source, &target).unwrap_or_else(|_| {
            panic!("CoordinateTransform could not be build from {from} and {to}.")
        });
        Self {
            transformation,
            source_reference: source,
            target_reference: target,
        }
    }
    /// Transforms a single (x, z) coordinate pair into the target CRS.
    pub fn transform_coordinates(&self, input_x: f64, input_z: f64) -> (f64, f64) {
        let mut output_x = [input_x];
        let mut output_z = [input_z];
        let mut whatever = [1.];
        self.transformation
            .transform_coords(&mut whatever, &mut output_x, &mut output_z)
            .expect("Could not successfully complete coordinate transform");
        (output_x[0], output_z[0])
    }
}

#[cfg(test)]
mod tests {
    use super::CoordinateTransform;

    #[test]
    fn identity_transform_returns_input() {
        // Transforming between identical CRSs must be a no-op.
        let transform = CoordinateTransform::new(4326, 4326);
        let (x, z) = transform.transform_coordinates(10.0, 20.0);
        assert!((x - 10.0).abs() < 1e-9, "expected 10, got {x}");
        assert!((z - 20.0).abs() < 1e-9, "expected 20, got {z}");
    }

    #[test]
    fn identity_transform_preserves_another_point() {
        let transform = CoordinateTransform::new(3857, 3857);
        let (x, z) = transform.transform_coordinates(1_800_000.0, 6_100_000.0);
        assert!(
            (x - 1_800_000.0).abs() < 1e-6,
            "expected 1_800_000, got {x}"
        );
        assert!(
            (z - 6_100_000.0).abs() < 1e-6,
            "expected 6_100_000, got {z}"
        );
    }
}
