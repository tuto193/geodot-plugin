mod dataset_position_data;
mod geo_raster;

use std::str::FromStr;

use gdal::{
    Dataset, DriverManager,
    errors::GdalError,
    raster::{Buffer, GdalDataType, GdalType, ResampleAlg, StatisticsMinMax},
};
use godot::meta::ToGodot;

pub use dataset_position_data::DatasetPositionData;
pub use geo_raster::{Format, GeoRaster};

use crate::{geo_image::Interpolation, global::ExtentData};

/// Stateless helper for extracting tiles and metadata from GDAL datasets.
pub struct RasterTileExtractor;

impl RasterTileExtractor {
    // Intended public API surface: registers all GDAL drivers before use.
    #[allow(dead_code)]
    /// Register all GDAL drivers. Must be called once before using GDAL.
    pub fn init() {
        DriverManager::register_all();
    }

    /// Extract a tile of `image_size` pixels covering `size_meters` from the
    /// dataset, starting at the given top-left world coordinate.
    pub fn get_tile_from_dataset(
        dataset: &Dataset,
        top_left_x: f64,
        top_left_y: f64,
        size_meters: f64,
        image_size: i64,
        interpolation_type: Interpolation,
    ) -> Result<GeoRaster<'_>, GdalError> {
        Self::clip_dataset(
            dataset,
            top_left_x,
            top_left_y,
            size_meters,
            image_size,
            ResampleAlg::from_str(&interpolation_type.to_godot().to_string())?,
        )
    }

    /// Write `values` into the dataset at the pixel closest to the given world
    /// center coordinate.
    // TODO: Re-check this function in the future to confirm correct functionality
    pub fn write_into_dataset<T: Copy + GdalType>(
        dataset: &mut Dataset,
        center_x: f64,
        center_y: f64,
        values: &[T],
        // scale: f64, // unused
        // interpolation_type: ResampleAlg, // unused
    ) -> Result<(), GdalError> {
        let position_data = DatasetPositionData::new(dataset, center_x, center_y, 0.)?;
        let data_type = dataset.rasterband(1)?.band_type();
        match data_type {
            GdalDataType::Float32 | GdalDataType::Float64 => {
                let mut values: Buffer<T> = Buffer::new((1, 1), values.into());
                let mut band = dataset.rasterband(1)?;
                band.write::<T>(
                    (
                        position_data.pixels_x as isize,
                        position_data.pixels_y as isize,
                    ),
                    (1, 1),
                    &mut values,
                )?;
                return Ok(());
            }
            GdalDataType::UInt8 => {
                let bands = dataset.rasterbands();
                // let band_count = bands.count();
                for (i, band) in bands.enumerate() {
                    let mut band = band?;
                    let mut value: Buffer<T> = Buffer::new((1, 1), vec![values[i]]);
                    band.write::<T>(
                        (
                            position_data.pixels_x as isize,
                            position_data.pixels_y as isize,
                        ),
                        (1, 1),
                        &mut value,
                    )?;
                }
            }
            x => {
                return Err(GdalError::BadArgument(format!("{x}")));
            }
        }
        Ok(())
    }
    // pub fn smooth_add_into_dataset(
    //     dataset: &mut Dataset,
    //     center_x: f64,
    //     center_y: f64,
    //     summand: f64,
    //     radius: f64,
    // ) {
    // }

    /// Return the geographic extent (bounds) of the dataset.
    pub fn get_extent_data(dataset: &Dataset) -> Result<ExtentData, GdalError> {
        let transform = dataset.geo_transform()?;
        let raster_size = dataset.raster_size();
        Ok(ExtentData {
            left: transform[0] + 0.0 * transform[1] + 0.0 * transform[2],
            right: transform[0] + raster_size.0 as f64 * transform[1] + 0.0 * transform[2],
            top: transform[3] + 0.0 * transform[4] + 0.0 * transform[5],
            down: transform[3] + 0.0 * transform[4] + raster_size.1 as f64 * transform[5],
        })
    }

    /// Compute the minimum and maximum values of the dataset's first band.
    pub fn get_min_max(dataset: &Dataset) -> Result<StatisticsMinMax, GdalError> {
        let rb = dataset.rasterband(1)?;
        rb.compute_raster_min_max(true)
    }
    // pub fn get_min(dataset: &Dataset) -> f64 {}
    // pub fn get_max(dataset: &Dataset) -> f64 {}

    /// Return the world-space size of a single pixel.
    pub fn get_pixel_size(dataset: &Dataset) -> Result<f64, GdalError> {
        let transform = dataset.geo_transform()?;
        Ok(transform[1])
    }

    fn clip_dataset(
        dataset: &Dataset,
        top_left_x: f64,
        top_left_y: f64,
        size_meters: f64,
        image_size: i64,
        interpolation_type: ResampleAlg,
    ) -> Result<GeoRaster<'_>, GdalError> {
        let position_data = DatasetPositionData::new(dataset, top_left_x, top_left_y, size_meters)?;
        GeoRaster::new(
            dataset,
            position_data.pixels_x,
            position_data.pixels_y,
            position_data.size_pixels,
            image_size,
            interpolation_type,
        )
    }
}
