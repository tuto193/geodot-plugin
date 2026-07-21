use std::{cell::RefCell, rc::Rc};

use godot::prelude::*;

use crate::vector_extractor::{Feature, PolygonFeature};

use super::geo_feature::GeoFeature;

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoPolygon {
    base: Base<RefCounted>,
    gdal_feature: Option<Rc<RefCell<PolygonFeature>>>,
}

#[godot_api]
impl GeoPolygon {
    #[signal]
    fn feature_changed();
    #[func]
    pub fn get_float_offset_outer_vertices(
        &self,
        offset_x: f64,
        offset_y: f64,
    ) -> PackedVector2Array {
        if let Some(ref gdal_feature) = self.gdal_feature
            && let Some(polygon_feature) = gdal_feature
                .borrow()
                .as_any()
                .downcast_ref::<PolygonFeature>()
            && let Some(outer_vertices) = polygon_feature.get_outer_vertices()
        {
            return outer_vertices
                .iter()
                .map(|&v| Vector2::new((v.0 + offset_x) as f32, (v.1 + offset_y) as f32))
                .collect();
        }
        PackedVector2Array::new()
    }
    #[func]
    pub fn get_offset_outer_vertices(&self, offset_x: i64, offset_y: i64) -> PackedVector2Array {
        self.get_float_offset_outer_vertices(offset_x as f64, offset_y as f64)
    }
    #[func]
    pub fn get_outer_vertices(&self) -> PackedVector2Array {
        self.get_offset_outer_vertices(0, 0)
    }
    #[func]
    pub fn set_float_offset_outer_vertices(
        &mut self,
        offset_x: f64,
        offset_y: f64,
        vertices: PackedVector2Array,
    ) {
        if let Some(ref gdal_feature) = self.gdal_feature {
            let new_outer_vertices: Vec<(f64, f64, f64)> = vertices
                .as_slice()
                .iter()
                .map(|v| (v.x as f64 + offset_x, v.y as f64 + offset_y, 0.0))
                .collect();

            gdal_feature
                .borrow_mut()
                .set_outer_vertices(new_outer_vertices);

            self.emit_feature_changed();
        }
    }
    #[func]
    pub fn set_offset_outer_vertices(
        &mut self,
        offset_x: i64,
        offset_y: i64,
        vertices: PackedVector2Array,
    ) {
        self.set_float_offset_outer_vertices(offset_x as f64, offset_y as f64, vertices);
    }
    #[func]
    pub fn set_outer_vertices(&mut self, vertices: PackedVector2Array) {
        self.set_offset_outer_vertices(0, 0, vertices);
    }

    #[func]
    pub fn add_hole(&mut self, hole: PackedVector2Array) {
        if let Some(ref gdal_feature) = self.gdal_feature {
            let vertices: Vec<(f64, f64, f64)> = hole
                .as_slice()
                .iter()
                .map(|v| (v.x as f64, v.y as f64, 0.0))
                .collect();

            gdal_feature.borrow_mut().add_hole(vertices);

            self.emit_feature_changed();
        }
    }
    #[func]
    pub fn get_float_offset_holes(
        &self,
        offset_x: f64,
        offset_y: f64,
    ) -> Array<PackedVector2Array> {
        if let Some(ref gdal_feature) = self.gdal_feature
            && let Some(polygon) = gdal_feature
                .borrow()
                .as_any()
                .downcast_ref::<PolygonFeature>()
        {
            return polygon
                .get_holes()
                .iter()
                .map(|raw_hole_vertices| {
                    raw_hole_vertices
                        .iter()
                        .map(|&vertex| {
                            Vector2::new((vertex.0 + offset_x) as f32, (vertex.1 + offset_y) as f32)
                        })
                        .collect()
                })
                .collect();
        }
        Array::new()
    }
    #[func]
    pub fn get_offset_holes(&self, offset_x: i64, offset_y: i64) -> Array<PackedVector2Array> {
        self.get_float_offset_holes(offset_x as f64, offset_y as f64)
    }
    #[func]
    pub fn get_holes(&self) -> Array<PackedVector2Array> {
        self.get_offset_holes(0, 0)
    }
}

#[godot_dyn]
impl GeoFeature<PolygonFeature> for GeoPolygon {
    fn gdal_feature(&self) -> Option<Rc<RefCell<PolygonFeature>>> {
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
    fn set_gdal_feature(&mut self, gdal_feature: Rc<RefCell<PolygonFeature>>) {
        self.gdal_feature = Some(Rc::clone(&gdal_feature))
    }
    fn set_deleted(&mut self, is_deleted: bool) {
        // gdal_feature.borrow_mut().set_deleted(is_deleted);
        if let Some(ref mut gdal_feature) = self.gdal_feature {
            gdal_feature.borrow_mut().set_deleted(is_deleted);
        }
    }
}
