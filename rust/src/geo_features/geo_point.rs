use std::{cell::RefCell, rc::Rc};

use godot::prelude::*;

use crate::vector_extractor::{Feature, PointFeature};

use super::geo_feature::GeoFeature;

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoPoint {
    base: Base<RefCounted>,
    gdal_feature: Option<Rc<RefCell<PointFeature>>>,
}

// Internally, a different coordinate system is used (Z up and reversed), which is why the
// coordinates are passed in a different order here.
#[godot_api]
impl GeoPoint {
    #[signal]
    fn feature_changed();
    #[func]
    pub fn get_float_offset_vector3(&self, offset_x: f64, offset_y: f64, offset_z: f64) -> Vector3 {
        if let Some(ref gdal_feature) = self.gdal_feature
            && let Some(point_feature) = gdal_feature
                .borrow()
                .as_any()
                .downcast_ref::<PointFeature>()
        {
            let point = point_feature.get_vector();
            return Vector3::new(
                (point.0 + offset_x) as f32,
                (point.2 + offset_y) as f32,
                (-point.1 - offset_z) as f32,
            );
        }
        Vector3::ZERO
    }
    #[func]
    pub fn get_vector3(&self) -> Vector3 {
        self.get_offset_vector3(0, 0, 0)
    }

    #[func]
    pub fn get_offset_vector3(&self, offset_x: i64, offset_y: i64, offset_z: i64) -> Vector3 {
        self.get_float_offset_vector3(offset_x as f64, offset_y as f64, offset_z as f64)
    }
    #[func]
    pub fn set_float_offset_vector3(
        &mut self,
        vector: Vector3,
        offset_x: f64,
        offset_y: f64,
        offset_z: f64,
    ) {
        if let Some(ref mut point_feature) = self.gdal_feature.as_mut() {
            let new_vector: (f64, f64, f64) = (
                offset_x + vector.x as f64,
                offset_z - vector.z as f64,
                offset_y + vector.y as f64,
            );

            point_feature.borrow_mut().set_vector(new_vector);
            self.emit_feature_changed();
        }
    }
    #[func]
    pub fn set_offset_vector3(
        &mut self,
        vector: Vector3,
        offset_x: i64,
        offset_y: i64,
        offset_z: i64,
    ) {
        self.set_float_offset_vector3(vector, offset_x as f64, offset_y as f64, offset_z as f64);
    }
    #[func]
    pub fn set_vector3(&mut self, vector: Vector3) {
        self.set_offset_vector3(vector, 0, 0, 0);
    }
}

#[godot_dyn]
impl GeoFeature<PointFeature> for GeoPoint {
    fn gdal_feature(&self) -> Option<Rc<RefCell<PointFeature>>> {
        if let Some(ref gdal_feature) = self.gdal_feature {
            return Some(Rc::clone(gdal_feature));
        }
        None
    }
    fn emit_feature_changed(&mut self) {
        self.signals().feature_changed().emit();
    }
    fn set_attribute(&mut self, name: GString, value: GString) {
        // TODO: better error handling
        if let Some(ref mut gdal_feature) = self.gdal_feature {
            gdal_feature
                .borrow_mut()
                .set_attribute(name.into(), value.into());
            self.emit_feature_changed();
        }
    }
    fn set_gdal_feature(&mut self, gdal_feature: Rc<RefCell<PointFeature>>) {
        self.gdal_feature = Some(Rc::clone(&gdal_feature))
    }
    fn set_deleted(&mut self, is_deleted: bool) {
        // gdal_feature.borrow_mut().set_deleted(is_deleted);
        if let Some(ref mut gdal_feature) = self.gdal_feature {
            gdal_feature.borrow_mut().set_deleted(is_deleted);
        }
    }
}
