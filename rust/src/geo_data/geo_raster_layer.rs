use godot::{
    classes::{Image, image},
    prelude::*,
};

use crate::{
    geo_data::geo_dataset::GeoDataset,
    geo_image::{GeoImage, Interpolation},
    global::ExtentData,
    raster_tile_extractor::{self, Format, GeoRaster, RasterTileExtractor},
    vector_extractor::{NativeDataset, VectorExtractor},
};
use std::{cell::RefCell, path::Path, rc::Rc};

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoRasterLayer {
    base: Base<RefCounted>,
    origin_geodataset: Option<Gd<GeoDataset>>,
    native_dataset: Option<Rc<RefCell<NativeDataset>>>,
    extent_data: Option<ExtentData>,
    #[var(pub)]
    name: GString,
    #[var]
    write_access: bool,
}

#[godot_api]
impl GeoRasterLayer {
    /// Returns true if the layer could successfully be loaded.
    #[func]
    pub fn is_valid(&self) -> bool {
        self.native_dataset
            .as_ref()
            .is_some_and(|ds| ds.borrow().is_valid())
    }

    /// Returns true for read-write-access and false for read-only-access.
    #[func]
    pub fn has_write_access(&self) -> bool {
        self.write_access
    }

    /// Returns information about this file:
    /// `is_subdataset`: true if this layer is part of a larger dataset (e.g. a GeoPackage) and not a single file (e.g. a GeoTIFF).
    /// `path`: the path to the underlying dataset (e.g. the GeoTIFF, or the GeoPackage which this layer was opened from).
    /// `name`: the name of the layer (if `is_subdataset` is true) or the name of the file (otherwise).
    #[func]
    pub fn get_file_info(&self) -> VarDictionary {
        let mut result = Dictionary::new();
        if !self.is_valid() {
            godot_error!("Can't get file info from invalid GeoRasterLayer.");
            return result;
        }
        // If this dataset comes from another dataset as a subdataset, the origin_dataset is set.
        result.set("is_subdataset", self.origin_geodataset.is_some());
        // If origin_dataset is not set, this means that this layer is not a subdataset; therefore, the
        // name should only be the file name without the path. Otherwise, if this is a subdataset, the
        // name is just the name.
        // If origin_dataset is not set, the name equals the path. Otherwise, the path comes from the
        // origin_dataset and is set in the `path` attribute.
        let (name, path) = if self.origin_geodataset.is_none() {
            (self.name.get_file().get_basename(), self.name.clone())
        } else {
            (
                self.name.clone(),
                self.native_dataset
                    .as_ref()
                    .expect("GeoRasterLayer.dataset should not be NULL at this point")
                    .borrow()
                    .path
                    .to_str()
                    .expect("Dataset.path should consist only of valid UTF-8")
                    .to_godot(),
            )
        };

        result.set("name", &name.to_variant());
        result.set("path", &path.to_variant());

        result
    }

    /// Returns the spatial reference of the underlying dataset.
    #[func]
    pub fn get_epsg_code(&self) -> i64 {
        if !self.is_valid() {
            godot_error!("Can't get EPSG code from invalid GeoRasterLayer");
            -1
        } else {
            self.native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid Dataset")
                .borrow()
                .get_epsg_code()
                .unwrap_or_else(|| {
                    godot_error!("EPSG Code from Dataset in GeoRasterLayer could not be retrieved");
                    -2
                }) as i64
        }
    }

    /// Returns the Image format which corresponds to the data within this raster layer.
    #[func]
    pub fn get_format(&self) -> image::Format {
        if !self.is_valid() {
            godot_error!("Cannot get format from invalid GeoRasterLayer");
            return image::Format::MAX;
        }
        // self is valid, so self.native_dataset(.gdal_dataset) exist(s)
        match GeoRaster::get_format_for_gdal_dataset(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
        ) {
            Ok(format) => match format {
                raster_tile_extractor::Format::Byte => image::Format::R8,
                raster_tile_extractor::Format::Rf => image::Format::RF,
                raster_tile_extractor::Format::Rgb => image::Format::RGB8,
                raster_tile_extractor::Format::Rgba => image::Format::RGBA8,
                raster_tile_extractor::Format::Mixed | raster_tile_extractor::Format::Unknown => {
                    image::Format::MAX
                }
            },
            Err(err) => {
                godot_error!("Wasn't able to get format for Dataset {err}");
                image::Format::MAX
            }
        }
    }

    /// @brief Get the total amount of raster bands contained in the layer.
    /// Returns 0 if layer is not valid
    /// @return the total amount of raster bands in the layer.
    #[func]
    pub fn get_band_count(&self) -> i64 {
        if !self.is_valid() {
            godot_error!("Can't get band count in invalid GeoRasterLayer!");
            return 0;
        }
        self.native_dataset
            .as_ref()
            .expect("GeoRasterLayer should have a valid dataset")
            .borrow()
            .gdal_dataset
            .as_ref()
            .expect("GeoRasterLayer should have a valid dataset")
            .borrow()
            .rasterbands()
            .count() as i64
    }

    /// Returns the descriptions of the individual raster bands as strings in an array.
    #[func]
    pub fn get_band_descriptions(&self) -> Array<GString> {
        if !self.is_valid() {
            godot_error!("Can't get band descriptions in invalid GeoRasterLayer!");
            return array![];
        }
        self.native_dataset
            .as_ref()
            .expect("GeoRasterLayer should have a valid dataset")
            .borrow()
            .get_raster_band_descriptions()
            .iter()
            .map(|d| d.to_godot())
            .collect()
    }

    /// Returns the dataset which this layer was opened from or `null` if it was opened directly, e.g.
    /// from a GeoTIFF.
    #[func]
    pub fn get_dataset(&self) -> Option<Gd<GeoDataset>> {
        if !self.is_valid() {
            godot_error!("Can't get band count in invalid GeoRasterLayer!");
            return None;
        }
        self.origin_geodataset.clone()
    }

    // Returns a clone of this layer which points to the same file, but uses a
    // different object to access it. This is required when using the same
    // layer in multiple threads.
    // RUST: Replaced with derive
    // Ref<GeoRasterLayer> clone();

    /// Returns a GeoImage corresponding to the given position and size.
    /// The requested section is read from this GeoRasterLayer into that GeoImage, so this
    /// operation is costly for large images. (Consider multithreading.)
    #[func]
    pub fn get_image(
        &self,
        top_left_x: f64,
        top_left_y: f64,
        size_meters: f64,
        image_size: i64,
        interpolation_type: Interpolation,
    ) -> Option<Gd<GeoImage>> {
        if !self.is_valid() {
            godot_error!("Can't get image in invalid GeoRasterLayer!");
            return None;
        }
        match RasterTileExtractor::get_tile_from_dataset(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
            top_left_x,
            top_left_y,
            size_meters,
            image_size,
            interpolation_type,
        ) {
            Ok(geo_raster) => {
                let mut result = GeoImage::new_gd();
                if let Err(e) = result
                    .bind_mut()
                    .set_raster(&geo_raster, interpolation_type)
                {
                    godot_error!("Could not set raster on GeoImage: {e}");
                    return None;
                }
                Some(result)
            }
            Err(e) => {
                godot_error!(
                    "{}:{}:get_immage -> get_tile_from_dataset returned an invalid raster: {e}",
                    file!(),
                    line!()
                );
                None
            }
        }
    }

    /// Like get_image but only returns the GeoImage of a single Band.
    /// Useful for datasets with many bands which are interpreted in an unusual way (i.e. not RGB(A)).
    #[func]
    pub fn get_band_image(
        &self,
        top_left_x: f64,
        top_left_y: f64,
        size_meters: f64,
        image_size: i64,
        interpolation_type: Interpolation,
        band_index: i64,
    ) -> Option<Gd<GeoImage>> {
        if !self.is_valid() {
            godot_error!(
                "{}:{}:get_band_image: Can't get band image in invalid GeoRasterLayer",
                file!(),
                line!()
            );
            return None;
        }
        match RasterTileExtractor::get_tile_from_dataset(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
            top_left_x,
            top_left_y,
            size_meters,
            image_size,
            interpolation_type,
        ) {
            Ok(raster) => {
                let mut result = GeoImage::new_gd();
                if let Err(e) = result.bind_mut().set_raster_from_band(
                    &raster,
                    interpolation_type,
                    band_index as usize,
                ) {
                    godot_error!("Could not set raster band on GeoImage: {e}");
                    return None;
                }
                Some(result)
            }
            Err(e) => {
                godot_error!(
                    "Error in {}:{}:get_band_image:get_tile_from_dataset: returned an invalid raster {e}",
                    file!(),
                    line!()
                );
                None
            }
        }
    }

    /// Returns the value in the GeoRasterLayer at exactly the given position.
    /// Note that when reading many values from a confined area, it is more efficient to call
    /// get_image and read the pixels from there.
    #[func]
    pub fn get_value_at_position(&self, pos_x: f64, pos_y: f64) -> real {
        if !self.is_valid() {
            godot_error!("Can't get valie in invalid GeoRasterLayer!");
            return 0.;
        }
        self.get_value_at_position_with_resolution(pos_x, pos_y, 0.0001)
    }

    /// Returns the value in the GeoRasterLayer at exactly the given position, at the given
    /// resolution. This is useful for getting heights which match up with the heights of a lower
    /// resolution image which was fetched using `get_image`, rather than being as detailed as
    /// possible.
    #[func]
    pub fn get_value_at_position_with_resolution(
        &self,
        pos_x: f64,
        pos_y: f64,
        pixel_size_meters: f64,
    ) -> real {
        if !self.is_valid() {
            godot_error!("Can't get valu in invalid GeoRasterLayer!");
            return 0.;
        }
        // TODO: Figure out what exactly we need to clamp to for precise values
        // let pos_x = pos_x - pos_x.fposmod(pixel_size_meters);
        // let pos_y = pos_y - pos_y.fposmod(pixel_size_meters);
        match RasterTileExtractor::get_tile_from_dataset(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
            pos_x,
            pos_y,
            pixel_size_meters,
            1,
            Interpolation::try_from_usize(1).expect("1 is a valid Interpolation index"),
        ) {
            Ok(raster) => {
                // TODO: Currently only implemented for RF type.
                // For others, we would either need a completely generic return value, or other specific
                // functions (as the user likely knows or wants to know the exact type).
                if raster.get_format() == Format::Rf {
                    match raster.get_as_array::<f32>() {
                        Ok(as_array) => {
                            if let Some(first_element) = as_array.into_iter().next() {
                                return first_element;
                            }
                            godot_error!(
                                "Error in {}:{}:get_value_at_position_with_resolution. Array was empty.",
                                file!(),
                                line!()
                            );
                        }
                        Err(e) => {
                            godot_error!(
                                "Error in {}:{}:get_value_at_position_with_resolution. Cannot get array as f32 -> {e}",
                                file!(),
                                line!()
                            );
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!(
                    "Error in {}:{}:get_value_at_position_with_resolution. Unable to get_tile_from dataset -> {e} ",
                    file!(),
                    line!()
                );
            }
        }
        -1.0
    }

    /// Replaces exactly one pixel at the given position with the given value.
    /// The value must correspond to this layer's type (e.g. a float for Float32 images and a Color
    /// for RGB images).
    /// Useful for modifying datasets on a small scale, e.g. correcting land-use values.
    ///
    /// Returns true on success. False otherwise.
    #[func]
    pub fn set_value_at_position(&mut self, pos_x: f64, pos_y: f64, value: Variant) -> bool {
        if !self.is_valid() {
            godot_error!("Can't set value in invalid GeoRasterLayer");
            return false;
        }
        let wrote_successfully: bool = match (value.get_type(), self.get_format()) {
            (VariantType::FLOAT, image::Format::RF) => {
                let godot_float = value.to::<real>();
                let values = vec![godot_float];
                RasterTileExtractor::write_into_dataset(
                    &mut self
                        .native_dataset
                        .as_mut()
                        .expect("GeoRasterLayer should have a valid dataset")
                        .borrow_mut()
                        .gdal_dataset
                        .as_mut()
                        .expect("GeoRasterLayer should have a valid dataset")
                        .borrow_mut(),
                    pos_x,
                    pos_y,
                    &values,
                    // 1.0,
                    // ResampleAlg::iter().nth(0).unwrap(),
                )
                .map_or_else(
                    |e| {
                        godot_error!("Could not write FLOAT into dataset {e}");
                        false
                    },
                    |_| true,
                )
            }
            (VariantType::COLOR, image::Format::RGB8 | image::Format::RGBA8) => {
                let color: Color = value.to();
                let values = vec![color.r * 255.0, color.g * 255.0, color.b * 255.0];
                RasterTileExtractor::write_into_dataset(
                    &mut self
                        .native_dataset
                        .as_mut()
                        .expect("GeoRasterLayer should have a valid dataset")
                        .borrow_mut()
                        .gdal_dataset
                        .as_mut()
                        .expect("GeoRasterLayer should have a valid dataset")
                        .borrow_mut(),
                    pos_x,
                    pos_y,
                    &values,
                    // 1.0,
                    // ResampleAlg::iter().nth(0).unwrap(),
                )
                .map_or_else(
                    |e| {
                        godot_error!("Could not write Color into dataset: {e}");
                        false
                    },
                    |_| true,
                )
            }
            (VariantType::INT, image::Format::R8) => {
                // TODO: Should we check for precision loss and emit a warning here?
                let godot_int: i64 = value.to();
                // Downcast to i8 (char), since there are no int images
                let values = vec![godot_int as i8];
                RasterTileExtractor::write_into_dataset(
                    &mut self
                        .native_dataset
                        .as_mut()
                        .expect("GeoRasterLayer should have a valid dataset")
                        .borrow_mut()
                        .gdal_dataset
                        .as_mut()
                        .expect("GeoRasterLayer should have a valid dataset")
                        .borrow_mut(),
                    pos_x,
                    pos_y,
                    &values,
                    // 1.0,
                    // ResampleAlg::iter().nth(0).unwrap(),
                )
                .map_or_else(
                    |e| {
                        godot_error!("Could not write INT into dataset: {e}");
                        false
                    },
                    |_| true,
                )
            }
            (v_type, s_type) => {
                godot_error!(
                    "Type mismatch: value of type {v_type:?} and dataset of type {s_type:?}"
                );
                false
            }
        };
        if !wrote_successfully {
            return false;
        }
        self.native_dataset
            .as_mut()
            .expect("GeoRasterLayer should have a valid dataset")
            .borrow_mut()
            .gdal_dataset
            .as_mut()
            .expect("GeoRasterLayer should have a valid dataset")
            .borrow_mut()
            .flush_cache()
            .map_or_else(
                |e| {
                    godot_error!("Could not flush cache on gdal_dataset {e}");
                    false
                },
                |_| true,
            )
    }

    /// Adds the given summand to the previous value at the given position and smoothly cross-fades
    /// into the original data along the given radius.
    /// Useful for terraforming terrain, e.g. creating a new hill with smooth slopes.
    #[func]
    pub fn smooth_add_value_at_position(
        &mut self,
        pos_x: f64,
        pos_y: f64,
        summand: f64,
        radius: f64,
    ) {
        if !self.is_valid() {
            godot_error!("Can't set value in invalid GeoRasterLayer!");
            return;
        }

        // FIXME: Like overlay_image_at_position, this could be done much more efficiently by
        // batch-reading and writing.
        let resolution = self.get_pixel_size() as f64;
        if resolution <= 0.0 {
            godot_error!("Can't smoothly add values with a non-positive pixel size!");
            return;
        }

        let mut offset_x = -radius;
        while offset_x <= radius {
            let mut offset_y = -radius;
            while offset_y <= radius {
                let distance_to_center =
                    (offset_x * offset_x + offset_y * offset_y).sqrt() / radius;
                let summand_factor = 1.0 - distance_to_center;

                if summand_factor > 0.0 {
                    let pos_here_x = pos_x + offset_x;
                    let pos_here_y = pos_y + offset_y;

                    let existing_value = self.get_value_at_position(pos_here_x, pos_here_y) as f64;
                    let new_value = existing_value + summand_factor * summand;

                    self.set_value_at_position(pos_here_x, pos_here_y, new_value.to_variant());
                }

                offset_y += resolution;
            }
            offset_x += resolution;
        }
    }

    /// Adds the given image to the raster dataset at the given position. Fully opaque values are
    /// replaced; values with alpha between 0 and 1 are interpolated between original and new.
    /// A scale of 1 assumes that both images have the same resolution per meter; 2 means that one
    /// pixel in this image corresponds to two pixels in the dataset.
    /// Useful for drawing into datasets with custom brushes, e.g. a pre-defined land-use pattern.
    #[func]
    pub fn overlay_image_at_position(
        &mut self,
        pos_x: f64,
        pos_y: f64,
        mut image: Gd<Image>,
        scale: f64,
    ) {
        if !self.is_valid() {
            godot_error!("Can't overlay image in invalid GeoRasterLayer!");
            return;
        }

        // FIXME: Rough initial implementation - it works but is very inefficient. Rather than
        // repeatedly calling set_value_at_position, the entire image should be written at once.
        let resolution = self.get_pixel_size() as f64;
        if resolution <= 0.0 {
            godot_error!("Can't overlay an image with a non-positive pixel size!");
            return;
        }

        let target_size = (scale / resolution).ceil() as i32;
        image.resize(target_size, target_size);

        let data = image.get_data();
        let image_width = image.get_width();
        let image_height = image.get_height();
        if image_width <= 0 || image_height <= 0 {
            return;
        }

        let image_format = image.get_format();
        let layer_format = self.get_format();
        let byte_count = data.len();
        let slice = data.as_slice();

        match (image_format, layer_format) {
            (image::Format::R8, image::Format::R8) => {
                // Single-band byte
                for (i, &value) in slice.iter().enumerate() {
                    let pos_in_image_x =
                        pos_x + ((i as i32 % image_width) as f64 / image_width as f64) * scale;
                    let pos_in_image_y =
                        pos_y - ((i as i32 / image_width) as f64 / image_height as f64) * scale;
                    self.set_value_at_position(
                        pos_in_image_x,
                        pos_in_image_y,
                        (value as i64).to_variant(),
                    );
                }
            }
            (image::Format::RGB8, image::Format::RGB8 | image::Format::RGBA8) => {
                // RGB
                for i in (0..byte_count).step_by(3) {
                    let value_r = slice[i] as f32 / 255.0;
                    let value_g = slice[i + 1] as f32 / 255.0;
                    let value_b = slice[i + 2] as f32 / 255.0;
                    let pixel = i as i32 / 3;
                    let pos_in_image_x =
                        pos_x + ((pixel % image_width) as f64 / image_width as f64) * scale;
                    let pos_in_image_y =
                        pos_y - ((pixel / image_width) as f64 / image_height as f64) * scale;
                    self.set_value_at_position(
                        pos_in_image_x,
                        pos_in_image_y,
                        Color::from_rgb(value_r, value_g, value_b).to_variant(),
                    );
                }
            }
            (image::Format::RGBA8, image::Format::RGB8 | image::Format::RGBA8) => {
                // RGBA
                for i in (0..byte_count).step_by(4) {
                    let value_r = slice[i] as f32 / 255.0;
                    let value_g = slice[i + 1] as f32 / 255.0;
                    let value_b = slice[i + 2] as f32 / 255.0;
                    let value_a = slice[i + 3] as f32 / 255.0;
                    let pixel = i as i32 / 4;
                    let pos_in_image_x =
                        pos_x + ((pixel % image_width) as f64 / image_width as f64) * scale;
                    let pos_in_image_y =
                        pos_y - ((pixel / image_width) as f64 / image_height as f64) * scale;
                    self.set_value_at_position(
                        pos_in_image_x,
                        pos_in_image_y,
                        Color::from_rgba(value_r, value_g, value_b, value_a).to_variant(),
                    );
                }
            }
            (image::Format::RF, image::Format::RF) => {
                // 4-byte float
                for i in (0..byte_count).step_by(4) {
                    let value =
                        f32::from_ne_bytes([slice[i], slice[i + 1], slice[i + 2], slice[i + 3]]);
                    let pos_in_image_x =
                        pos_x + ((i as i32 % image_width) as f64 / image_width as f64) * scale;
                    let pos_in_image_y =
                        pos_y - ((i as i32 / image_width) as f64 / image_height as f64) * scale;
                    self.set_value_at_position(pos_in_image_x, pos_in_image_y, value.to_variant());
                }
            }
            (image_format, layer_format) => {
                godot_error!(
                    "Type mismatch: image of type {image_format:?} and dataset of type {layer_format:?}"
                );
            }
        }
    }

    /// Returns the extent of the layer in projected meters (assuming it is rectangular).
    #[func]
    pub fn get_extent(&self) -> Rect2 {
        if !self.is_valid() {
            godot_error!("Can't get extent in invalid GeoRasterLayer!");
            return Rect2::default();
        }
        let Some(extent_data) = self.extent_data.as_ref() else {
            return Rect2::default();
        };
        Rect2::from_components(
            extent_data.left as f32,
            extent_data.top as f32,
            (extent_data.right - extent_data.left) as f32,
            (extent_data.down - extent_data.top) as f32,
        )
    }

    /// Returns the point in the center of the layer in projected meters.
    /// The y-component is 0.0.
    #[func]
    pub fn get_center(&self) -> Vector3 {
        if !self.is_valid() {
            godot_error!("Can't get center in invalid GeoRasterLayer!");
            return Vector3::ZERO;
        }
        let Some(extent_data) = self.extent_data.as_ref() else {
            return Vector3::ZERO;
        };
        Vector3::new(
            (extent_data.left + (extent_data.right - extent_data.left) / 2.0) as f32,
            0.0,
            (extent_data.top + (extent_data.down - extent_data.top) / 2.0) as f32,
        )
    }

    /// Returns the smallest value found in the first raster band of the dataset.
    /// Note that this requires the dataset to have pre-computed statistics!
    #[func]
    pub fn get_min(&self) -> f32 {
        if !self.is_valid() {
            godot_error!("Can't get min in invalid GeoRasterLayer!");
            return 0.0;
        }
        match RasterTileExtractor::get_min_max(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
        ) {
            Ok(min_max) => min_max.min as f32,
            Err(e) => {
                godot_error!("Could not get min value in GeoRasterLayer: {e}");
                0.0
            }
        }
    }

    /// Returns the largest value found in the first raster band of the dataset.
    /// Note that this requires the dataset to have pre-computed statistics!
    #[func]
    pub fn get_max(&self) -> f32 {
        if !self.is_valid() {
            godot_error!("Can't get max in invalid GeoRasterLayer!");
            return 0.0;
        }
        match RasterTileExtractor::get_min_max(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
        ) {
            Ok(min_max) => min_max.max as f32,
            Err(e) => {
                godot_error!("Could not get max value in GeoRasterLayer: {e}");
                0.0
            }
        }
    }

    /// Returns the length of a side of a pixel in the dataset, in meters.
    #[func]
    pub fn get_pixel_size(&self) -> f32 {
        if !self.is_valid() {
            godot_error!("Can't get pixel size in invalid GeoRasterLayer!");
            return 0.0;
        }
        match RasterTileExtractor::get_pixel_size(
            &self
                .native_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow()
                .gdal_dataset
                .as_ref()
                .expect("GeoRasterLayer should have a valid dataset")
                .borrow(),
        ) {
            Ok(pixel_size) => pixel_size as f32,
            Err(e) => {
                godot_error!("Could not get pixel size in GeoRasterLayer: {e}");
                0.0
            }
        }
    }

    /// Load a raster dataset file such as a GeoTIFF into this object.
    #[func]
    pub fn load_from_file(&mut self, file_path: GString, write_access: bool) {
        self.write_access = write_access;
        let path = Path::new(&file_path.to_string()).to_path_buf();
        match VectorExtractor::open_dataset(&path, write_access) {
            Ok(dataset) => self.set_native_dataset(dataset),
            Err(err) => {
                godot_error!("Could not load GeoRasterLayer from path '{file_path}': {err}");
                return;
            }
        }
        // For backwards compatibility, the object's name is set to the full path.
        self.name = file_path;
    }

    /// Set the GDALDataset object for this layer. Must be a valid raster
    /// dataset. Not exposed to Godot since Godot doesn't know about
    /// GDALDatasets - this is only for internal use.
    pub fn set_native_dataset(&mut self, new_dataset: NativeDataset) {
        if let Some(gdal_dataset) = new_dataset.gdal_dataset.as_ref() {
            match RasterTileExtractor::get_extent_data(&gdal_dataset.borrow()) {
                Ok(extent_data) => self.extent_data = Some(extent_data),
                Err(e) => godot_error!("Could not compute extent data for GeoRasterLayer: {e}"),
            }
        }
        self.native_dataset = Some(Rc::new(RefCell::new(new_dataset)));
    }

    /// Sets the dataset which this layer was opened from.
    /// Not exposed to Godot since it should never construct GeoFeatureLayers by hand.
    pub fn set_origin_dataset(&mut self, dataset: Gd<GeoDataset>) {
        self.origin_geodataset = Some(dataset);
    }
}
