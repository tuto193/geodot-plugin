use std::{cell::RefCell, rc::Rc};

use godot::prelude::*;

use crate::vector_extractor::Feature;

/// Shared behavior for the concrete `Geo*` feature wrappers (point, line,
/// polygon). Provides default attribute/id accessors backed by the underlying
/// GDAL feature, while leaving the type-specific hooks to each implementor.
pub trait GeoFeature<T: Feature + ?Sized> {
    #[allow(dead_code, reason = "This API code meant to be called from GDScript.")]
    fn get_attribute(&self, name: GString) -> GString {
        if let Some(gdal_feature) = self.gdal_feature()
            && let Some(ref result) = gdal_feature.borrow().get_attribute(name.into())
        {
            return result.into();
        }
        "".into()
    }

    #[allow(dead_code, reason = "This API code meant to be called from GDScript.")]
    fn set_attribute(&mut self, name: GString, value: GString);

    #[allow(dead_code, reason = "This API code meant to be called from GDScript.")]
    fn get_id(&self) -> Option<u64> {
        if let Some(gdal_feature) = self.gdal_feature() {
            return gdal_feature.borrow().get_id();
        }
        None
    }

    #[allow(dead_code, reason = "This API code meant to be called from GDScript.")]
    fn get_attributes(&self) -> Option<Dictionary<GString, GString>> {
        if let Some(gdal_feature) = self.gdal_feature() {
            let attributes = gdal_feature.borrow().get_attributes();
            let result = attributes
                .iter()
                .map(|(k, v)| (k.to_godot(), v.to_godot()))
                .collect::<Dictionary<GString, GString>>();
            return Some(result);
        }
        None
    }

    fn set_gdal_feature(&mut self, gdal_feature: Rc<RefCell<T>>);

    fn set_deleted(&mut self, is_deleted: bool);

    fn gdal_feature(&self) -> Option<Rc<RefCell<T>>>;

    fn emit_feature_changed(&mut self);
}
