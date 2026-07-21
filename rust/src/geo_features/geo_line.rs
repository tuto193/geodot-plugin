use std::{cell::RefCell, rc::Rc};

use godot::{classes::Curve3D, prelude::*};

use crate::vector_extractor::{Feature, LineFeature};

use super::geo_feature::GeoFeature;

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoLine {
    base: Base<RefCounted>,
    gdal_feature: Option<Rc<RefCell<LineFeature>>>,
}

#[godot_api]
impl GeoLine {
    #[signal]
    fn feature_changed();

    #[func]
    pub fn get_float_offset_curve3d(
        &self,
        offset_x: f64,
        offset_y: f64,
        offset_z: f64,
    ) -> Gd<Curve3D> {
        let mut result = Curve3D::new_gd();
        if let Some(ref gdal_feature) = self.gdal_feature
            && let Some(line) = gdal_feature.borrow().as_any().downcast_ref::<LineFeature>()
        {
            let total_points = line.get_point_count();
            for i in 0..total_points {
                let point = line.get_line_point(i);
                result.add_point(Vector3::new(
                    point.0 as f32 + offset_x as f32,
                    point.2 as f32 + offset_y as f32,
                    -point.1 as f32 - offset_z as f32,
                ));
            }
        }
        result
    }

    #[func]
    pub fn get_offset_curve3d(&self, offset_x: i64, offset_y: i64, offset_z: i64) -> Gd<Curve3D> {
        self.get_float_offset_curve3d(offset_x as f64, offset_y as f64, offset_z as f64)
    }

    #[func]
    pub fn get_curve3d(&self) -> Gd<Curve3D> {
        self.get_offset_curve3d(0, 0, 0)
    }

    #[func]
    pub fn set_float_offset_curve3d(
        &mut self,
        curve: Gd<Curve3D>,
        offset_x: f64,
        offset_y: f64,
        offset_z: f64,
    ) {
        let point_count = curve.get_point_count();
        if let Some(ref mut line) = self.gdal_feature {
            line.borrow_mut().set_point_count(point_count as usize);
            for i in 0..point_count {
                let point = curve.get_point_position(i);
                let offset_point = (
                    point.x as f64 + offset_x,
                    point.z as f64 + offset_z,
                    -point.y as f64 - offset_y,
                );
                line.borrow_mut().set_line_point(i as usize, offset_point);
            }
        }
        self.emit_feature_changed();
    }

    #[func]
    pub fn set_offset_curve3d(
        &mut self,
        curve: Gd<Curve3D>,
        offset_x: i64,
        offset_y: i64,
        offset_z: i64,
    ) {
        self.set_float_offset_curve3d(curve, offset_x as f64, offset_y as f64, offset_z as f64);
    }

    #[func]
    pub fn set_curve3d(&mut self, curve: Gd<Curve3D>) {
        self.set_float_offset_curve3d(curve, 0., 0., 0.)
    }

    #[func]
    pub fn add_point(&mut self, point: Vector3) {
        if let Some(ref mut line) = self.gdal_feature {
            let new_count = line.borrow_mut().get_point_count() + 1;
            line.borrow_mut().set_point_count(new_count);
            line.borrow_mut().set_line_point(
                new_count - 1,
                (point.x as f64, point.y as f64, point.z as f64),
            );
        }
        self.emit_feature_changed();
    }
}

#[godot_dyn]
impl GeoFeature<LineFeature> for GeoLine {
    fn gdal_feature(&self) -> Option<Rc<RefCell<LineFeature>>> {
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
    fn set_gdal_feature(&mut self, gdal_feature: Rc<RefCell<LineFeature>>) {
        self.gdal_feature = Some(Rc::clone(&gdal_feature))
    }
    fn set_deleted(&mut self, is_deleted: bool) {
        // gdal_feature.borrow_mut().set_deleted(is_deleted);
        if let Some(ref mut gdal_feature) = self.gdal_feature {
            gdal_feature.borrow_mut().set_deleted(is_deleted);
        }
    }
}
