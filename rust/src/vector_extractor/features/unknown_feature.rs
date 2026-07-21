use super::feature::{Feature, FromOgrFeature, GeometryType};
use gdal::vector::Feature as OGRFeature;
use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone)]
pub struct UnknownFeature {
    geometry_type: GeometryType,
    fid: Option<u64>,
    /// All field values, converted to strings at construction time.
    attributes: HashMap<String, String>,
    is_deleted: bool,
}

impl Feature for UnknownFeature {
    fn set_deleted(&mut self, deleted: bool) {
        self.is_deleted = deleted;
    }

    fn is_deleted(&self) -> bool {
        self.is_deleted
    }

    fn geometry_type(&self) -> GeometryType {
        self.geometry_type
    }

    fn get_id(&self) -> Option<u64> {
        self.fid
    }

    fn get_attributes(&self) -> HashMap<String, String> {
        self.attributes.clone()
    }

    fn get_attribute(&self, field_name: String) -> Option<String> {
        self.attributes.get(&field_name).cloned()
    }

    /// Updates the in-memory attribute. Changes are not written back to the
    /// source dataset until a save operation is performed.
    fn set_attribute(&mut self, name: String, value: String) {
        self.attributes.insert(name, value);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FromOgrFeature for UnknownFeature {
    fn from_feature(feature: OGRFeature<'_>) -> Self {
        let geometry_type = if feature.geometry().is_some() {
            GeometryType::Unknown
        } else {
            GeometryType::None
        };

        let fid = feature.fid();

        // Drain all fields into an owned map; skip null / non-string-convertible values.
        let attributes = feature
            .fields()
            .filter_map(|(name, val)| val.and_then(|v| v.into_string().map(|s| (name, s))))
            .collect();

        Self {
            geometry_type,
            fid,
            attributes,
            is_deleted: false,
        }
    }
}

impl PartialEq for UnknownFeature {
    fn eq(&self, other: &Self) -> bool {
        self.geometry_type == other.geometry_type
            && self.is_deleted == other.is_deleted
            && self.fid == other.fid
    }
}

impl Eq for UnknownFeature {}

impl Hash for UnknownFeature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.geometry_type.hash(state);
        self.is_deleted.hash(state);
        self.fid.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_extractor::features::test_util::with_feature;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    fn hash_of(feature: &UnknownFeature) -> u64 {
        let mut hasher = DefaultHasher::new();
        feature.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn geometry_type_is_unknown_when_geometry_present() {
        with_feature(Some("POINT (1 1)"), &[], |feature| {
            let unknown = UnknownFeature::from_feature(feature);
            assert_eq!(unknown.geometry_type(), GeometryType::Unknown);
        });
    }

    #[test]
    fn geometry_type_is_none_when_geometry_absent() {
        with_feature(None, &[], |feature| {
            let unknown = UnknownFeature::from_feature(feature);
            assert_eq!(unknown.geometry_type(), GeometryType::None);
        });
    }

    #[test]
    fn extracts_and_overwrites_attributes() {
        with_feature(None, &[("key", "value")], |feature| {
            let mut unknown = UnknownFeature::from_feature(feature);
            assert_eq!(
                unknown.get_attribute("key".to_string()),
                Some("value".to_string())
            );
            unknown.set_attribute("key".to_string(), "updated".to_string());
            assert_eq!(
                unknown.get_attribute("key".to_string()),
                Some("updated".to_string())
            );
        });
    }

    #[test]
    fn deletion_flag_round_trips() {
        with_feature(None, &[], |feature| {
            let mut unknown = UnknownFeature::from_feature(feature);
            assert!(!unknown.is_deleted());
            unknown.set_deleted(true);
            assert!(unknown.is_deleted());
        });
    }

    #[test]
    fn equality_and_hashing_are_consistent() {
        with_feature(Some("POINT (1 1)"), &[], |feature| {
            let a = UnknownFeature::from_feature(feature);
            let b = a.clone();
            assert!(a == b);
            assert_eq!(hash_of(&a), hash_of(&b));

            let mut c = a.clone();
            c.set_deleted(true);
            assert!(a != c);
        });
    }
}
