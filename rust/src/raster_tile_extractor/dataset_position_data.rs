use gdal::{Dataset, errors::GdalError};

pub struct DatasetPositionData {
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub meters_x: f64,
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub meters_y: f64,
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub size_meters: f64,
    pub pixels_x: i64,
    pub pixels_y: i64,
    pub size_pixels: i64,
}

impl DatasetPositionData {
    pub fn new(
        dataset: &Dataset,
        meters_x: f64,
        meters_y: f64,
        size_meters: f64,
    ) -> Result<Self, GdalError> {
        let transform = dataset.geo_transform()?;
        let previous_meters_x = transform[0];
        let previous_meters_y = transform[3];
        let pixel_size = transform[1];
        let offset_meters_x = meters_x - previous_meters_x;
        let offset_meters_y = previous_meters_y - meters_y;
        let pixels_x = (offset_meters_x / pixel_size) as i64;
        let pixels_y = (offset_meters_y / pixel_size) as i64;
        let size_pixels = (size_meters / pixel_size).ceil() as i64;
        Ok(Self {
            size_pixels,
            size_meters,
            meters_x,
            meters_y,
            pixels_x,
            pixels_y,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdal::DriverManager;

    fn mem_dataset_with_transform() -> Dataset {
        let mut dataset = DriverManager::get_driver_by_name("MEM")
            .expect("the in-memory 'MEM' driver should be available")
            .create_with_band_type::<u8, _>("", 100, 100, 1)
            .expect("could not create in-memory raster");
        // Origin (1000, 2000) with a 10-unit pixel size (north-up).
        dataset
            .set_geo_transform(&[1000.0, 10.0, 0.0, 2000.0, 0.0, -10.0])
            .expect("could not set geo transform");
        dataset
    }

    #[test]
    fn computes_pixel_offsets_from_geotransform() {
        let dataset = mem_dataset_with_transform();
        let data = DatasetPositionData::new(&dataset, 1100.0, 1900.0, 50.0).unwrap();
        assert_eq!(data.pixels_x, 10); // (1100 - 1000) / 10
        assert_eq!(data.pixels_y, 10); // (2000 - 1900) / 10
        assert_eq!(data.size_pixels, 5); // ceil(50 / 10)
    }

    #[test]
    fn rounds_size_up_to_whole_pixels() {
        let dataset = mem_dataset_with_transform();
        let data = DatasetPositionData::new(&dataset, 1000.0, 2000.0, 45.0).unwrap();
        assert_eq!(data.pixels_x, 0);
        assert_eq!(data.pixels_y, 0);
        assert_eq!(data.size_pixels, 5); // ceil(45 / 10)
    }

    #[test]
    fn errors_without_geotransform() {
        // A dataset without an explicit geo transform cannot resolve pixel
        // positions, so construction fails cleanly instead of panicking.
        let dataset = DriverManager::get_driver_by_name("MEM")
            .unwrap()
            .create_with_band_type::<u8, _>("", 10, 10, 1)
            .unwrap();
        assert!(DatasetPositionData::new(&dataset, 0.0, 0.0, 1.0).is_err());
    }
}
