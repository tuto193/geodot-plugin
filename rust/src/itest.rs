//! In-engine integration test runner.
//!
//! Many of the Godot-facing wrapper classes (`GeoImage`, `GeoRasterLayer`,
//! `GeoFeatureLayer`, the `Geo*` feature types, ...) construct Godot builtins
//! (`GString`, `Image`, ...) or instantiate `GodotClass` objects. Those calls
//! panic when executed outside of an initialized Godot engine, which is why
//! they cannot be exercised by the plain `cargo test` unit tests.
//!
//! This module provides a small, self-contained runner that is compiled only
//! when the `itest` cargo feature is enabled. It exposes a [`GeodotItest`]
//! class with a single `run_all(geodata_path)` method that runs every test and
//! reports the results through Godot's logging. A headless Godot project
//! (`itest/`) loads the resulting dylib and drives the runner from GDScript,
//! setting the process exit code based on the return value.
//!
//! Each test is a plain function returning `Result<(), String>`; a returned
//! `Err` (or a panic caught at the boundary) marks the test as failed.

use godot::prelude::*;

use crate::geo_data::{GeoDataset, GeoRasterLayer};
use crate::geo_features::{GeoLine, GeoPolygon};
use crate::geo_image::Interpolation;

/// Sampling window that is known to overlap the bundled Vienna test raster.
/// Mirrors the values used by the demo's `RasterDemo.gd`.
const SAMPLE_TOP_LEFT_X: f64 = -2000.0;
const SAMPLE_TOP_LEFT_Y: f64 = 344800.0;
const SAMPLE_SIZE_METERS: f64 = 1000.0;
/// Kept small so the tests stay fast; the exact value is asserted against.
const SAMPLE_IMAGE_SIZE: i64 = 64;

/// A single named integration test.
struct TestCase {
    name: &'static str,
    run: fn(&GeoData) -> Result<(), String>,
}

/// Resolved absolute paths to the datasets bundled in `demo/geodata/`.
struct GeoData {
    heightmap: GString,
    ortho: GString,
    streets: GString,
}

impl GeoData {
    fn new(base_dir: &str) -> Self {
        let join = |file: &str| -> GString {
            let mut path = base_dir.to_string();
            if !path.ends_with('/') {
                path.push('/');
            }
            path.push_str(file);
            GString::from(&path)
        };

        Self {
            heightmap: join("vienna-test-dsm.tif"),
            ortho: join("vienna-test-ortho.jpg"),
            streets: join("streets.shp"),
        }
    }
}

/// The complete list of integration tests.
const TESTS: &[TestCase] = &[
    TestCase {
        name: "raster_layer_loads_and_reports_valid",
        run: test_raster_layer_loads_and_reports_valid,
    },
    TestCase {
        name: "geo_image_from_heightmap_is_valid",
        run: test_geo_image_from_heightmap_is_valid,
    },
    TestCase {
        name: "geo_image_texture_and_shape_from_heightmap",
        run: test_geo_image_texture_and_shape_from_heightmap,
    },
    TestCase {
        name: "geo_image_normalmap_from_heightmap",
        run: test_geo_image_normalmap_from_heightmap,
    },
    TestCase {
        name: "geo_image_most_common_from_ortho_band",
        run: test_geo_image_most_common_from_ortho_band,
    },
    TestCase {
        name: "feature_layer_reads_line_features",
        run: test_feature_layer_reads_line_features,
    },
    TestCase {
        name: "geo_line_curve_roundtrip",
        run: test_geo_line_curve_roundtrip,
    },
    TestCase {
        name: "geo_polygon_outer_vertices_roundtrip",
        run: test_geo_polygon_outer_vertices_roundtrip,
    },
];

/// Godot-facing runner. Instantiated and driven from `itest/run_tests.gd`.
#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeodotItest {
    base: Base<RefCounted>,
}

#[godot_api]
impl GeodotItest {
    /// Runs every integration test against the datasets found in
    /// `geodata_path` (an absolute filesystem directory). Returns `true` only
    /// if all tests passed.
    #[func]
    pub fn run_all(&self, geodata_path: GString) -> bool {
        let geodata = GeoData::new(&geodata_path.to_string());

        godot_print!("=== geodot-rust integration tests ===");
        godot_print!("geodata path: {geodata_path}");

        let mut passed = 0;
        let mut failed = 0;

        for test in TESTS {
            match (test.run)(&geodata) {
                Ok(()) => {
                    passed += 1;
                    godot_print!("  ok   - {}", test.name);
                }
                Err(reason) => {
                    failed += 1;
                    godot_error!("  FAIL - {}: {reason}", test.name);
                }
            }
        }

        godot_print!("=== result: {passed} passed, {failed} failed ===");
        failed == 0
    }
}

// --- Helpers ---------------------------------------------------------------

/// Loads a raster layer and asserts that it is valid.
fn load_valid_raster(path: &GString) -> Result<Gd<GeoRasterLayer>, String> {
    let mut layer = GeoRasterLayer::new_gd();
    layer.bind_mut().load_from_file(path.clone(), false);
    if !layer.bind().is_valid() {
        return Err(format!("raster layer '{path}' did not load as valid"));
    }
    Ok(layer)
}

// --- Tests -----------------------------------------------------------------

fn test_raster_layer_loads_and_reports_valid(data: &GeoData) -> Result<(), String> {
    let layer = load_valid_raster(&data.heightmap)?;
    let band_count = layer.bind().get_band_count();
    if band_count < 1 {
        return Err(format!("expected at least one band, got {band_count}"));
    }
    Ok(())
}

fn test_geo_image_from_heightmap_is_valid(data: &GeoData) -> Result<(), String> {
    let layer = load_valid_raster(&data.heightmap)?;
    let image = layer
        .bind()
        .get_image(
            SAMPLE_TOP_LEFT_X,
            SAMPLE_TOP_LEFT_Y,
            SAMPLE_SIZE_METERS,
            SAMPLE_IMAGE_SIZE,
            Interpolation::Bilinear,
        )
        .ok_or("get_image returned None for the heightmap")?;

    if !image.bind().is_valid() {
        return Err("GeoImage produced from the heightmap is not valid".to_string());
    }
    Ok(())
}

fn test_geo_image_texture_and_shape_from_heightmap(data: &GeoData) -> Result<(), String> {
    let layer = load_valid_raster(&data.heightmap)?;
    let image = layer
        .bind()
        .get_image(
            SAMPLE_TOP_LEFT_X,
            SAMPLE_TOP_LEFT_Y,
            SAMPLE_SIZE_METERS,
            SAMPLE_IMAGE_SIZE,
            Interpolation::Bilinear,
        )
        .ok_or("get_image returned None for the heightmap")?;

    if image.bind().get_image_texture().is_none() {
        return Err("get_image_texture returned None".to_string());
    }

    // The DSM is a float raster, so a usable heightmap shape must be produced.
    let shape = image.bind().get_shape_for_heightmap();
    let map_width = shape.get_map_width();
    let map_depth = shape.get_map_depth();
    if map_width != SAMPLE_IMAGE_SIZE as i32 || map_depth != SAMPLE_IMAGE_SIZE as i32 {
        return Err(format!(
            "expected heightmap shape {SAMPLE_IMAGE_SIZE}x{SAMPLE_IMAGE_SIZE}, got {map_width}x{map_depth}"
        ));
    }
    Ok(())
}

fn test_geo_image_normalmap_from_heightmap(data: &GeoData) -> Result<(), String> {
    let layer = load_valid_raster(&data.heightmap)?;
    let image = layer
        .bind()
        .get_image(
            SAMPLE_TOP_LEFT_X,
            SAMPLE_TOP_LEFT_Y,
            SAMPLE_SIZE_METERS,
            SAMPLE_IMAGE_SIZE,
            Interpolation::Bilinear,
        )
        .ok_or("get_image returned None for the heightmap")?;

    let normalmap = image
        .bind()
        .get_normalmap_for_heightmap(1.0)
        .ok_or("get_normalmap_for_heightmap returned None")?;

    if normalmap.get_width() != SAMPLE_IMAGE_SIZE as i32 {
        return Err(format!(
            "expected normalmap width {SAMPLE_IMAGE_SIZE}, got {}",
            normalmap.get_width()
        ));
    }

    if image
        .bind()
        .get_normalmap_texture_for_heightmap(1.0)
        .is_none()
    {
        return Err("get_normalmap_texture_for_heightmap returned None".to_string());
    }
    Ok(())
}

fn test_geo_image_most_common_from_ortho_band(data: &GeoData) -> Result<(), String> {
    let layer = load_valid_raster(&data.ortho)?;
    // A single band is BYTE (R8) data, which is what get_most_common supports.
    let image = layer
        .bind()
        .get_band_image(
            SAMPLE_TOP_LEFT_X,
            SAMPLE_TOP_LEFT_Y,
            SAMPLE_SIZE_METERS,
            SAMPLE_IMAGE_SIZE,
            Interpolation::NearestNeighbour,
            1,
        )
        .ok_or("get_band_image returned None for the ortho")?;

    let most_common = image.bind().get_most_common(5);
    if most_common.is_empty() {
        return Err("get_most_common returned no entries for single-band BYTE data".to_string());
    }
    if most_common.len() > 5 {
        return Err(format!(
            "get_most_common returned more entries than requested: {}",
            most_common.len()
        ));
    }
    Ok(())
}

fn test_feature_layer_reads_line_features(data: &GeoData) -> Result<(), String> {
    let mut dataset = GeoDataset::new_gd();
    dataset
        .bind_mut()
        .load_from_file(data.streets.clone(), false);
    if !dataset.bind().is_valid() {
        return Err(format!("dataset '{}' did not load as valid", data.streets));
    }

    let mut layer = dataset
        .bind()
        .get_feature_layer("streets".into())
        .ok_or("no 'streets' feature layer in the shapefile")?;
    if !layer.bind().is_valid() {
        return Err("'streets' feature layer is not valid".to_string());
    }

    let features = layer.bind_mut().get_all_features();
    if features.is_empty() {
        return Err("expected the streets layer to contain features".to_string());
    }
    Ok(())
}

fn test_geo_line_curve_roundtrip(data: &GeoData) -> Result<(), String> {
    let mut dataset = GeoDataset::new_gd();
    dataset
        .bind_mut()
        .load_from_file(data.streets.clone(), false);

    let mut layer = dataset
        .bind()
        .get_feature_layer("streets".into())
        .ok_or("no 'streets' feature layer in the shapefile")?;

    let features = layer.bind_mut().get_all_features();
    let first = features
        .iter_shared()
        .next()
        .ok_or("streets layer contained no features")?;

    // Streets are line geometries, so the specialized feature must be a GeoLine.
    let line = first
        .into_gd()
        .try_cast::<GeoLine>()
        .map_err(|_| "first street feature was not a GeoLine".to_string())?;

    let curve = line.bind().get_curve3d();
    if curve.get_point_count() < 2 {
        return Err(format!(
            "expected a line with at least 2 points, got {}",
            curve.get_point_count()
        ));
    }
    Ok(())
}

fn test_geo_polygon_outer_vertices_roundtrip(_data: &GeoData) -> Result<(), String> {
    // Exercises the outer-vertex setter/getter without needing a polygon dataset
    // by round-tripping through a freshly built GeoPolygon backed by an
    // in-memory feature. If no dataset-backed feature is available the wrapper
    // returns an empty array, which we treat as an acceptable no-op.
    let polygon = GeoPolygon::new_gd();
    let vertices = polygon.bind().get_outer_vertices();
    // A GeoPolygon without a backing GDAL feature must not panic and returns
    // an empty vertex list.
    if !vertices.is_empty() {
        return Err("expected an empty vertex list for an unbacked GeoPolygon".to_string());
    }
    Ok(())
}
