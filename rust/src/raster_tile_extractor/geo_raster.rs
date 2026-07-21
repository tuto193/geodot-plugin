use gdal::{
    Dataset as GdalDataset,
    errors::GdalError,
    raster::{GdalDataType, GdalType, ResampleAlg},
};

struct RasterIOHelper {
    pub clamped_pixel_offset_x: i64,
    pub clamped_pixel_offset_y: i64,

    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub min_raster_size: i64,
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub remainder_x_left: i64,
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub remainder_y_top: i64,
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub target_height: i64,
    #[allow(
        dead_code,
        reason = "TODO: needs to be investigated, why it isn't used in C++"
    )]
    pub target_width: i64,
    pub usable_height: i64,
    pub usable_width: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Rgb,
    Rgba,
    Rf,
    Byte,
    Mixed,
    Unknown,
}

pub struct GeoRaster<'a> {
    data: &'a GdalDataset,
    format: Format,
    pixel_offset_x: i64,
    pixel_offset_y: i64,
    source_window_size_pixels: i64,
    destination_window_size_pixels: i64,
    interpolation_type: ResampleAlg,
}

impl<'a> GeoRaster<'a> {
    pub fn new(
        data: &'a GdalDataset,
        pixel_offset_x: i64,
        pixel_offset_y: i64,
        source_window_size_pixels: i64,
        destination_window_size_pixels: i64,
        interpolation_type: ResampleAlg,
    ) -> Result<Self, GdalError> {
        let format = Self::get_format_for_gdal_dataset(data)?;
        Ok(Self {
            data,
            format,
            pixel_offset_x,
            pixel_offset_y,
            source_window_size_pixels,
            destination_window_size_pixels,
            interpolation_type,
        })
    }

    pub fn get_format(&self) -> Format {
        self.format
    }

    fn get_raster_io_helper(&self) -> RasterIOHelper {
        let pixel_offset_x = self.pixel_offset_x;
        let pixel_offset_y = self.pixel_offset_y;
        let source_window_pixels = self.source_window_size_pixels;
        let destination_window_size_pixels = self.destination_window_size_pixels;
        let available_raster_size = self.data.raster_size();
        let min_raster_size = available_raster_size.0.min(available_raster_size.1) as i64;
        let source_destination_ratio =
            destination_window_size_pixels as f64 / source_window_pixels as f64;
        let mut usable_width = source_window_pixels;
        let mut usable_height = source_window_pixels;
        let mut clamped_pixel_offset_x = pixel_offset_x;
        let mut clamped_pixel_offset_y = pixel_offset_y;

        let mut remainder_x_left = 0;
        let mut remainder_y_top = 0;

        let mut target_width = destination_window_size_pixels;
        let mut target_height = destination_window_size_pixels;

        let available_x = available_raster_size.0 as i64;
        let available_y = available_raster_size.1 as i64;

        let is_left_outside = pixel_offset_x < 0;
        let is_right_outside = pixel_offset_x + source_window_pixels > available_x;

        if is_left_outside {
            usable_width += pixel_offset_x;
            remainder_x_left = ((-pixel_offset_x) as f64 * source_destination_ratio) as i64;
            clamped_pixel_offset_x = 0;
            target_width = (usable_width as f64 * source_destination_ratio) as i64;
        }
        if is_right_outside {
            usable_width -= pixel_offset_x + source_window_pixels - available_x;
            target_width = (usable_width as f64 * source_destination_ratio) as i64;
        }
        if !is_left_outside && !is_right_outside {
            target_width = destination_window_size_pixels;
        }

        let is_up_outside = pixel_offset_y < 0;
        let is_down_outside = pixel_offset_y + source_window_pixels > available_y;
        if is_up_outside {
            usable_height += pixel_offset_y;
            remainder_y_top = ((-pixel_offset_y) as f64 * source_destination_ratio) as i64;
            clamped_pixel_offset_y = 0;
            target_height = (usable_height as f64 * source_destination_ratio) as i64;
        }
        if is_down_outside {
            usable_height -= pixel_offset_y + source_window_pixels - available_y;
            target_height = (usable_height as f64 * source_destination_ratio) as i64;
        }
        if !is_up_outside && !is_down_outside {
            target_height = destination_window_size_pixels;
        }

        RasterIOHelper {
            clamped_pixel_offset_x,
            clamped_pixel_offset_y,
            min_raster_size,
            remainder_y_top,
            remainder_x_left,
            target_width,
            target_height,
            usable_width,
            usable_height,
        }
    }

    pub fn get_as_array<T: Clone + Default + Copy + GdalType>(&self) -> Result<Vec<T>, GdalError> {
        let helper = self.get_raster_io_helper();
        let interpolation = if self.destination_window_size_pixels < self.source_window_size_pixels
        {
            ResampleAlg::NearestNeighbour
        } else {
            self.interpolation_type
        };

        let size = (self.get_pixel_size_x() * self.get_pixel_size_y()) as usize;
        let mut result = vec![T::default(); size];

        if helper.usable_width <= 0 || helper.usable_height <= 0 {
            return Ok(result);
        }

        match self.format {
            Format::Rf => {
                let band = self.data.rasterband(1)?;
                band.read_into_slice::<T>(
                    (
                        helper.clamped_pixel_offset_x as isize,
                        helper.clamped_pixel_offset_y as isize,
                    ),
                    (helper.usable_width as usize, helper.usable_height as usize),
                    (
                        self.get_pixel_size_x() as usize,
                        self.get_pixel_size_y() as usize,
                    ),
                    &mut result,
                    Some(interpolation),
                )?;
            }
            Format::Rgb | Format::Rgba | Format::Byte => {
                let total_bands = match self.format {
                    Format::Rgba => 4,
                    Format::Rgb => 3,
                    Format::Byte => 1,
                    _ => 1,
                };

                for band_number in 1..=total_bands {
                    let band = self.data.rasterband(band_number)?;
                    let band_size = (self.get_pixel_size_x() * self.get_pixel_size_y()) as usize;
                    let mut band_slice = vec![T::default(); band_size];

                    band.read_into_slice::<T>(
                        (
                            helper.clamped_pixel_offset_x as isize,
                            helper.clamped_pixel_offset_y as isize,
                        ),
                        (helper.usable_width as usize, helper.usable_height as usize),
                        (
                            self.get_pixel_size_x() as usize,
                            self.get_pixel_size_y() as usize,
                        ),
                        &mut band_slice,
                        Some(interpolation),
                    )?;

                    for (i, val) in band_slice.iter().enumerate() {
                        let insert_at = (band_number - 1) + i * total_bands;
                        if insert_at < result.len() {
                            result[insert_at] = *val;
                        }
                    }
                }
            }
            _ => {
                return Err(GdalError::BadArgument(
                    "GdalDataset format cannot be returned as array.".to_string(),
                ));
            }
        }
        Ok(result)
    }

    pub fn get_band_as_array<T: Clone + Default + Copy + GdalType>(
        &self,
        band_index: usize,
    ) -> Result<Vec<T>, GdalError> {
        let helper = self.get_raster_io_helper();
        let interpolation = if self.destination_window_size_pixels < self.source_window_size_pixels
        {
            ResampleAlg::NearestNeighbour
        } else {
            self.interpolation_type
        };

        let pixel_size = (self.get_pixel_size_x() * self.get_pixel_size_y()) as usize;
        let mut result = vec![T::default(); pixel_size];

        if helper.usable_width <= 0 || helper.usable_height <= 0 {
            return Ok(result);
        }

        match self.get_band_format(band_index) {
            Format::Byte | Format::Rf => {
                let band = self.data.rasterband(band_index)?;
                band.read_into_slice::<T>(
                    (
                        helper.clamped_pixel_offset_x as isize,
                        helper.clamped_pixel_offset_y as isize,
                    ),
                    (helper.usable_width as usize, helper.usable_height as usize),
                    (
                        self.get_pixel_size_x() as usize,
                        self.get_pixel_size_y() as usize,
                    ),
                    &mut result,
                    Some(interpolation),
                )?;
            }
            _ => {
                return Err(GdalError::BadArgument(
                    "Band format not supported".to_string(),
                ));
            }
        }
        Ok(result)
    }

    // Can be commented out, since rust solves the problem differently.
    // pub fn get_size_in_bytes(&self) -> i64 {
    //     let pixel_size = self.get_pixel_size_x() * self.get_pixel_size_y();
    //     match self.format {
    //         Format::Rgb => pixel_size * 3,
    //         Format::Rgba => pixel_size * 4,
    //         Format::Rf => pixel_size * 4,
    //         Format::Byte => pixel_size,
    //         _ => 0,
    //     }
    // }

    pub fn get_format_for_gdal_dataset(dataset: &GdalDataset) -> Result<Format, GdalError> {
        let raster_count = dataset.raster_count();
        let first_raster_type = dataset.rasterband(1)?.band_type();
        let raster_types_mismatch = !dataset.rasterbands().all(|rb| {
            rb.map(|b| b.band_type() == first_raster_type)
                .unwrap_or(false)
        });

        match first_raster_type {
            GdalDataType::UInt8 => match raster_count {
                4 => Ok(Format::Rgba),
                3 => Ok(Format::Rgb),
                _ => {
                    if raster_types_mismatch {
                        Ok(Format::Mixed)
                    } else {
                        Ok(Format::Byte)
                    }
                }
            },
            GdalDataType::Float32 | GdalDataType::Float64 => {
                if raster_types_mismatch {
                    Ok(Format::Mixed)
                } else {
                    Ok(Format::Rf)
                }
            }
            _ => Ok(Format::Unknown),
        }
    }

    pub fn get_band_format(&self, band_index: usize) -> Format {
        match self.data.rasterband(band_index) {
            Err(_) => Format::Unknown,
            Ok(band) => match band.band_type() {
                GdalDataType::Unknown => Format::Unknown,
                GdalDataType::UInt8 => Format::Byte,
                GdalDataType::Float32 | GdalDataType::Float64 => Format::Rf,
                _ => Format::Unknown,
            },
        }
    }

    pub fn get_pixel_size_x(&self) -> i64 {
        self.destination_window_size_pixels
    }

    pub fn get_pixel_size_y(&self) -> i64 {
        self.destination_window_size_pixels
    }

    /// Builds a histogram of pixel values across the raster's data.
    #[allow(
        dead_code,
        reason = "Part of the intended public API surface, used by get_most_common."
    )]
    pub fn get_histogram(&self) -> Result<Vec<u64>, GdalError> {
        let magic_number = 11000;
        let mut result = vec![0u64; magic_number];

        if self.format == Format::Rf {
            let array = self.get_as_array::<f64>()?;
            for value in array {
                let value = value as u64;
                if (value as usize) < magic_number {
                    result[value as usize] += 1;
                }
            }
        } else {
            let array = self.get_as_array::<u8>()?;
            let step = match self.format {
                Format::Rgb => 3,
                Format::Rgba => 4,
                Format::Byte => 1,
                _ => 1,
            };

            for i in (0..array.len()).step_by(step) {
                if i < array.len() {
                    let value = array[i] as usize;
                    if value < magic_number {
                        result[value] += 1;
                    }
                }
            }
        }
        Ok(result)
    }

    /// Returns the `number_of_elements` most common values in the raster,
    /// most-frequent first.
    #[allow(
        dead_code,
        reason = "Part of the intended public API surface; the Godot-facing GeoImage computes this from the image data directly."
    )]
    pub fn get_most_common(&self, number_of_elements: i64) -> Result<Vec<i64>, GdalError> {
        let get_highest_index_of_value = |array: &mut [u64]| -> usize {
            let (max_index, _) = array
                .iter()
                .enumerate()
                .max_by_key(|(_, value)| **value)
                .unwrap_or((0, &0));
            max_index
        };

        let mut elements = vec![0i64; number_of_elements as usize];
        let mut histogram = self.get_histogram()?;

        for element in elements.iter_mut() {
            let highest_index = get_highest_index_of_value(&mut histogram);
            *element = highest_index as i64;
            histogram[highest_index] = 0;
        }
        Ok(elements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdal::DriverManager;

    fn mem_dataset<T: GdalType>(bands: usize) -> GdalDataset {
        DriverManager::get_driver_by_name("MEM")
            .expect("the in-memory 'MEM' driver should be available")
            .create_with_band_type::<T, _>("", 8, 8, bands)
            .expect("could not create in-memory raster")
    }

    #[test]
    fn detects_single_byte_band_as_byte() {
        let dataset = mem_dataset::<u8>(1);
        assert_eq!(
            GeoRaster::get_format_for_gdal_dataset(&dataset).unwrap(),
            Format::Byte
        );
    }

    #[test]
    fn detects_three_byte_bands_as_rgb() {
        let dataset = mem_dataset::<u8>(3);
        assert_eq!(
            GeoRaster::get_format_for_gdal_dataset(&dataset).unwrap(),
            Format::Rgb
        );
    }

    #[test]
    fn detects_four_byte_bands_as_rgba() {
        let dataset = mem_dataset::<u8>(4);
        assert_eq!(
            GeoRaster::get_format_for_gdal_dataset(&dataset).unwrap(),
            Format::Rgba
        );
    }

    #[test]
    fn detects_float_band_as_rf() {
        let dataset = mem_dataset::<f32>(1);
        assert_eq!(
            GeoRaster::get_format_for_gdal_dataset(&dataset).unwrap(),
            Format::Rf
        );
    }

    #[test]
    fn pixel_size_matches_destination_window() {
        let dataset = mem_dataset::<u8>(1);
        let raster = GeoRaster::new(&dataset, 0, 0, 8, 16, ResampleAlg::NearestNeighbour).unwrap();
        assert_eq!(raster.get_pixel_size_x(), 16);
        assert_eq!(raster.get_pixel_size_y(), 16);
        assert_eq!(raster.get_format(), Format::Byte);
    }

    #[test]
    fn get_band_format_reports_band_data_type() {
        let dataset = mem_dataset::<f32>(1);
        let raster = GeoRaster::new(&dataset, 0, 0, 8, 8, ResampleAlg::NearestNeighbour).unwrap();
        assert_eq!(raster.get_band_format(1), Format::Rf);
        // A non-existent band index resolves to Unknown rather than panicking.
        assert_eq!(raster.get_band_format(99), Format::Unknown);
    }
}
