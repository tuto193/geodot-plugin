use std::rc::Rc;

use godot::prelude::*;

use crate::vector_extractor::CoordinateTransform;

#[derive(GodotClass)]
#[class(init, base=Object)]
pub struct GeoTransform {
    transform: Option<Rc<CoordinateTransform>>,
}

#[godot_api]
impl GeoTransform {
    #[func]
    pub fn set_transform(&mut self, from: u32, to: u32) {
        self.transform = Some(Rc::new(CoordinateTransform::new(from, to)));
    }
    #[func]
    pub fn transform_coordinates(&self, coordinates: Vector3) -> Vector3 {
        if let Some(ref transform) = self.transform {
            let transformed =
                transform.transform_coordinates(coordinates.x as f64, coordinates.z as f64);
            return Vector3::new(transformed.0 as f32, 0.0, transformed.1 as f32);
        }
        Vector3::default()
    }
}
