use std::{cell::RefCell, rc::Rc};

use godot::prelude::*;

use crate::vector_extractor::{Feature, UnknownFeature};

use super::geo_feature::GeoFeature;

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct GeoGeneralFeature {
    base: Base<RefCounted>,
    gdal_feature: Option<Rc<RefCell<UnknownFeature>>>,
}

// Internally, a different coordinate system is used (Z up and reversed), which is why the
// coordinates are passed in a different order here.
#[godot_api]
impl GeoGeneralFeature {
    #[signal]
    fn feature_changed();
}

#[godot_dyn]
impl GeoFeature<UnknownFeature> for GeoGeneralFeature {
    fn gdal_feature(&self) -> Option<Rc<RefCell<UnknownFeature>>> {
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
    fn set_gdal_feature(&mut self, gdal_feature: Rc<RefCell<UnknownFeature>>) {
        self.gdal_feature = Some(Rc::clone(&gdal_feature))
    }
    fn set_deleted(&mut self, is_deleted: bool) {
        // gdal_feature.borrow_mut().set_deleted(is_deleted);
        if let Some(ref mut gdal_feature) = self.gdal_feature {
            gdal_feature.borrow_mut().set_deleted(is_deleted);
        }
    }
}
