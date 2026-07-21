use godot::classes::image::Format;
use std::sync::Mutex;

use crate::error::GeodotError;
use crate::raster_tile_extractor::{self, GeoRaster};
use godot::{
    classes::{HeightMapShape3D, Image, ImageTexture},
    prelude::*,
};

#[derive(GodotConvert, Var, Export, Default, Clone, Copy, Debug)]
#[godot(via=GString)]
pub enum Interpolation {
    #[default]
    NearestNeighbour,
    Bilinear,
    Cubic,
    CubicSpline,
    Lanczos,
    Average,
    Mode,
    // From here down, these all do not exist as ResampleAlg in Gdal (Rust)
    Max,
    Min,
    Med,
    Q1,
    Q2,
}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoImage {
    base: Base<RefCounted>,
    is_valid: bool,
    #[var]
    image: Gd<Image>,
    normalmap: Mutex<Option<Gd<Image>>>,
    interpolation: Interpolation,
}

// impl IntoI

impl Interpolation {
    pub fn iter() -> impl Iterator<Item = Self> {
        vec![
            Self::NearestNeighbour,
            Self::Bilinear,
            Self::Cubic,
            Self::CubicSpline,
            Self::Lanczos,
            Self::Average,
            Self::Mode,
            Self::Max,
            Self::Min,
            Self::Med,
            Self::Q1,
            Self::Q2,
        ]
        .into_iter()
    }

    pub fn try_from_usize(value: usize) -> Result<Self, GString> {
        if let Some(possible_val) = Self::iter().nth(value) {
            return Ok(possible_val);
        }
        Err(format!(
            "Value '{value}' is not a possible value from {:?}",
            Self::iter().collect::<Vec<Interpolation>>()
        )
        .to_godot())
    }
}

#[cfg(test)]
mod tests {
    use super::Interpolation;

    #[test]
    fn iter_yields_all_twelve_variants() {
        assert_eq!(Interpolation::iter().count(), 12);
    }

    #[test]
    fn try_from_usize_maps_valid_indices_in_order() {
        assert!(matches!(
            Interpolation::try_from_usize(0),
            Ok(Interpolation::NearestNeighbour)
        ));
        assert!(matches!(
            Interpolation::try_from_usize(1),
            Ok(Interpolation::Bilinear)
        ));
        assert!(matches!(
            Interpolation::try_from_usize(6),
            Ok(Interpolation::Mode)
        ));
        assert!(matches!(
            Interpolation::try_from_usize(11),
            Ok(Interpolation::Q2)
        ));
    }

    #[test]
    fn try_from_usize_returns_none_variant_boundary() {
        // Index 12 is out of range (only 0..=11 are valid). We check that
        // `iter().nth()` yields nothing for it without forcing construction of
        // the Godot-side error string, which requires an initialized engine.
        assert!(Interpolation::iter().nth(12).is_none());
    }
}

#[godot_api]
impl GeoImage {
    #[func]
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Import a GeoRaster and prepare the Godot Image
    /// Should not be called from the outside; Geodot only returns GeoImages
    /// which already have raster data.
    pub fn set_raster(
        &mut self,
        raster: &GeoRaster,
        interpolation: Interpolation,
    ) -> Result<(), GeodotError> {
        self.interpolation = interpolation;
        let image_size_x = raster.get_pixel_size_x() as i32;
        let image_size_y = raster.get_pixel_size_y() as i32;

        match raster.get_format() {
            raster_tile_extractor::Format::Rf => {
                let data = raster.get_as_array::<f32>()?;
                let byte_data: Vec<u8> = data.iter().flat_map(|&f| f.to_ne_bytes()).collect();

                if let Some(image) = Image::create_from_data(
                    image_size_x,
                    image_size_y,
                    false,
                    Format::RF,
                    &byte_data.into(),
                ) {
                    self.image = image;
                    self.is_valid = true;
                } else {
                    return Err(GeodotError::ImageCreation(
                        "Was not able to create image.".to_string(),
                    ));
                }
            }
            raster_tile_extractor::Format::Rgb => {
                let data = raster.get_as_array::<u8>()?;

                if let Some(image) = Image::create_from_data(
                    image_size_x,
                    image_size_y,
                    false,
                    Format::RGB8,
                    &data.into(),
                ) {
                    self.image = image;
                    self.is_valid = true;
                } else {
                    return Err(GeodotError::ImageCreation(
                        "Image could not be created from bytes".to_string(),
                    ));
                }
            }
            raster_tile_extractor::Format::Rgba => {
                let data = raster.get_as_array::<u8>()?;

                if let Some(image) = Image::create_from_data(
                    image_size_x,
                    image_size_y,
                    false,
                    Format::RGBA8,
                    &data.into(),
                ) {
                    self.image = image;
                    self.is_valid = true;
                } else {
                    return Err(GeodotError::ImageCreation(
                        "Image could not be created from bytes".to_string(),
                    ));
                }
            }
            raster_tile_extractor::Format::Byte => {
                let data = raster.get_as_array::<u8>()?;

                if let Some(image) = Image::create_from_data(
                    image_size_x,
                    image_size_y,
                    false,
                    Format::R8,
                    &data.into(),
                ) {
                    self.image = image;
                    self.is_valid = true;
                } else {
                    return Err(GeodotError::ImageCreation(
                        "Image could not be created from bytes".to_string(),
                    ));
                }
            }
            raster_tile_extractor::Format::Mixed => {
                return Err(GeodotError::UnsupportedFormat(
                    "Mixed format. Not yet supported.".to_string(),
                ));
            }
            raster_tile_extractor::Format::Unknown => {
                return Err(GeodotError::UnsupportedFormat(
                    "Unknown data type. Not supported.".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn xy_to_index(x: i32, y: i32, width: i32, height: i32) -> usize {
        let x = x.clamp(0, width - 1);
        let y = y.clamp(0, height - 1);
        (y * width + x) as usize
    }

    /// Like `set_raster` but uses only the band at band_index from raster.
    pub fn set_raster_from_band(
        &mut self,
        raster: &GeoRaster,
        interpolation: Interpolation,
        band_index: usize,
    ) -> Result<(), GeodotError> {
        self.interpolation = interpolation;
        let img_size_x = raster.get_pixel_size_x() as i32;
        let img_size_y = raster.get_pixel_size_y() as i32;

        let band_format = raster.get_band_format(band_index);
        match band_format {
            raster_tile_extractor::Format::Rf => {
                let data = raster.get_band_as_array::<f32>(band_index)?;
                let byte_data: Vec<u8> = data.iter().flat_map(|&f| f.to_ne_bytes()).collect();

                if let Some(image) = Image::create_from_data(
                    img_size_x,
                    img_size_y,
                    false,
                    Format::RF,
                    &byte_data.into(),
                ) {
                    self.image = image;
                    self.is_valid = true;
                    return Ok(());
                }
            }
            raster_tile_extractor::Format::Byte => {
                let data = raster.get_band_as_array::<u8>(band_index)?;

                if let Some(image) =
                    Image::create_from_data(img_size_x, img_size_y, false, Format::R8, &data.into())
                {
                    self.image = image;
                    self.is_valid = true;
                    return Ok(());
                }
            }
            _ => {
                return Err(GeodotError::UnsupportedFormat(
                    "Unknown format for band.".to_string(),
                ));
            }
        }
        Err(GeodotError::ImageCreation(
            "No image could be created from given parameters".to_string(),
        ))
    }

    #[func]
    /// Assuming the image is a heightmap, return the normal map corresponding
    /// to that heightmap. Somewhat costly because the generated heightmap has a
    /// higher precision than Godot's Image::bumpmap_to_normalmap.
    pub fn get_normalmap_for_heightmap(&self, scale: f32) -> Option<Gd<Image>> {
        let mut normal_map = self.normalmap.lock().expect("normalmap mutex was poisoned");

        let image = self.image.duplicate_resource();
        let width = image.get_width();
        let height = image.get_height();
        let mut normalmap_data = vec![0u8; (width * height * 4) as usize]; // RGBA

        for full_y in 0..height {
            for full_x in 0..width {
                // Prevent the edges from having flat normals by using the
                // closest valid normal
                let x = full_x.clamp(1, width - 2);
                let y = full_y.clamp(1, height - 2);

                // Sobel filter for getting the normal at this position
                let bottom_left = image.get_pixel(x + 1, y + 1).r;
                let bottom_center = image.get_pixel(x, y + 1).r;
                let bottom_right = image.get_pixel(x - 1, y + 1).r;

                let center_left = image.get_pixel(x + 1, y).r;
                let _center_center = image.get_pixel(x, y).r;
                let center_right = image.get_pixel(x - 1, y).r;

                let top_left = image.get_pixel(x + 1, y - 1).r;
                let top_center = image.get_pixel(x, y - 1).r;
                let top_right = image.get_pixel(x - 1, y - 1).r;

                let mut normal = Vector3::new(
                    (top_right + 2.0 * center_right + bottom_right)
                        - (top_left + 2.0 * center_left + bottom_left),
                    (bottom_left + 2.0 * bottom_center + bottom_right)
                        - (top_left + 2.0 * top_center + top_right),
                    1.0 / scale,
                );

                normal = normal.normalized();

                let index = Self::xy_to_index(full_x, full_y, width, height);
                normalmap_data[index * 4] = (127.5 + normal.x * 127.5) as u8;
                normalmap_data[index * 4 + 1] = (127.5 + normal.y * 127.5) as u8;
                normalmap_data[index * 4 + 2] = (127.5 + normal.z * 127.5) as u8;
                normalmap_data[index * 4 + 3] = 255;
            }
        }
        let result =
            Image::create_from_data(width, height, false, Format::RGBA8, &normalmap_data.into());
        *normal_map = result.clone();
        result
    }

    /// Get a Godot `ImageTexture` with the GeoImage's data.
    #[func]
    pub fn get_image_texture(&self) -> Option<Gd<ImageTexture>> {
        ImageTexture::create_from_image(&self.image)
    }

    /// Wrapper for `get_normalmap_for_heightmap` which directly provides an
    /// `ImageTexture` with the image.
    #[func]
    pub fn get_normalmap_texture_for_heightmap(&self, scale: f32) -> Option<Gd<ImageTexture>> {
        let normalmap = self.get_normalmap_for_heightmap(scale)?;
        ImageTexture::create_from_image(&normalmap)
    }

    /// Returns a `HeightMapShape3D` which can be used for colliding with terrain
    /// created from a heightmap image. In order to perfectly match the terrain,
    /// the rows and columns of vertices in the terrain mesh must match the
    /// GeoImage width and height exactly, and the terrain mesh must be
    /// constructed out of _regular quads_ since the `HeightMapShape3D` is
    /// implemented this way internally. Only returns something useful when the
    /// GeoImage is of type Float.
    #[func]
    pub fn get_shape_for_heightmap(&self) -> Gd<HeightMapShape3D> {
        let mut shape = HeightMapShape3D::new_gd();

        if self.image.get_format() == Format::RF {
            let width = self.image.get_width();
            let height = self.image.get_height();

            // The RF image stores one 32-bit float per pixel; reinterpret the
            // raw bytes back into floats for the height map.
            let data = self.image.get_data();
            let heights: PackedFloat32Array = data
                .as_slice()
                .chunks_exact(4)
                .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            shape.set_map_width(width);
            shape.set_map_depth(height);
            shape.set_map_data(&heights);
        }

        shape
    }

    /// Get the `number_of_entries` most common values in the image.
    /// Only functional for single-band BYTE data!
    #[func]
    pub fn get_most_common(&self, number_of_entries: i64) -> Array<i64> {
        let mut result = Array::new();

        // Mirrors the C++ behavior: only meaningful for single-band BYTE data.
        if self.image.get_format() != Format::R8 {
            return result;
        }

        let data = self.image.get_data();
        let mut histogram = [0u64; 256];
        for &value in data.as_slice() {
            histogram[value as usize] += 1;
        }

        for _ in 0..number_of_entries.max(0) {
            let (index, &count) = histogram
                .iter()
                .enumerate()
                .max_by_key(|(_, count)| **count)
                .unwrap_or((0, &0));

            // Once the remaining counts are all zero, there is nothing left to
            // report.
            if count == 0 {
                break;
            }

            // Don't return the same value twice on the next iteration.
            histogram[index] = 0;

            // An index of 0 means "nothing found" as a sentinel in the original
            // implementation, so it is skipped in the output.
            if index == 0 {
                continue;
            }

            result.push(index as i64);
        }

        result
    }
}
