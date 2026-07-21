use gdal::vector::Feature as OGRFeature;
use std::{any::Any, cell::RefCell, collections::hash_map::HashMap, hash::Hash, rc::Rc};

#[cfg(test)]
use mockall::automock;

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum GeometryType {
    None,
    Point,
    Line,
    MultiLine,
    Polygon,
    MultiPolygon,
    Unknown,
}

impl From<&str> for GeometryType {
    fn from(value: &str) -> Self {
        match value {
            "POINT" => Self::Point,
            "POLYGON" => Self::Polygon,
            "LINESTRING" => Self::Line,
            "MULTILINESTRING" => Self::MultiLine,
            "MULTIPOLYGON" => Self::MultiPolygon,
            _ => Self::Unknown,
        }
    }
}

pub struct DynFeatureHolder(pub Rc<RefCell<dyn Feature>>);

impl PartialEq for DynFeatureHolder {
    fn eq(&self, other: &Self) -> bool {
        let self_borrowed = self.0.borrow();
        let other_borrowed = other.0.borrow();

        if self_borrowed.geometry_type() != other_borrowed.geometry_type() {
            return false;
        }
        if self_borrowed.is_deleted() != other_borrowed.is_deleted() {
            return false;
        }
        if self_borrowed.get_id() != other_borrowed.get_id() {
            return false;
        }

        let others_attributes = other_borrowed.get_attributes();
        !self_borrowed.get_attributes().iter().any(|(k, v)| {
            if let Some(other_v) = others_attributes.get(k) {
                v != other_v
            } else {
                true
            }
        })
    }
}

impl Eq for DynFeatureHolder {}

impl Hash for DynFeatureHolder {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let borrowed = self.0.borrow();
        borrowed.geometry_type().hash(state);
        borrowed.is_deleted().hash(state);
        borrowed.get_id().hash(state);
        borrowed.get_attributes().iter().for_each(|(k, v)| {
            k.hash(state);
            v.hash(state);
        });
    }
}

#[cfg_attr(test, automock)]
pub trait Feature {
    fn as_any(&self) -> &dyn Any;

    fn set_deleted(&mut self, deleted: bool);
    fn is_deleted(&self) -> bool;

    fn get_attributes(&self) -> HashMap<String, String>;
    fn get_attribute(&self, field_name: String) -> Option<String>;
    fn set_attribute(&mut self, name: String, value: String);

    fn get_id(&self) -> Option<u64>;
    fn geometry_type(&self) -> GeometryType;
}

/// Construction of a concrete feature from a borrowed GDAL feature.
///
/// Kept separate from [`Feature`] so that the behavior trait stays free of the
/// borrowed `OGRFeature<'_>` argument (which makes it object-safe and mockable).
pub trait FromOgrFeature {
    /// Construct from a borrowed GDAL feature. All data is extracted eagerly so
    /// the resulting value owns everything it needs and carries no lifetime.
    fn from_feature(feature: OGRFeature<'_>) -> Self
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::{DynFeatureHolder, Feature, GeometryType, MockFeature};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::rc::Rc;

    /// Builds a mocked `Feature` (via mockall) with the given identity-relevant
    /// properties. Only the methods `DynFeatureHolder` relies on are stubbed.
    fn mock_feature(
        geometry_type: GeometryType,
        is_deleted: bool,
        id: Option<u64>,
        attributes: HashMap<String, String>,
    ) -> Rc<RefCell<dyn Feature>> {
        let mut mock = MockFeature::new();
        mock.expect_geometry_type()
            .times(..)
            .return_const(geometry_type);
        mock.expect_is_deleted().times(..).return_const(is_deleted);
        mock.expect_get_id().times(..).return_const(id);
        mock.expect_get_attributes()
            .times(..)
            .returning(move || attributes.clone());
        Rc::new(RefCell::new(mock))
    }

    fn hash_of(holder: &DynFeatureHolder) -> u64 {
        let mut hasher = DefaultHasher::new();
        holder.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn parses_known_geometry_types() {
        assert_eq!(GeometryType::from("POINT"), GeometryType::Point);
        assert_eq!(GeometryType::from("POLYGON"), GeometryType::Polygon);
        assert_eq!(GeometryType::from("LINESTRING"), GeometryType::Line);
        assert_eq!(
            GeometryType::from("MULTILINESTRING"),
            GeometryType::MultiLine
        );
        assert_eq!(
            GeometryType::from("MULTIPOLYGON"),
            GeometryType::MultiPolygon
        );
    }

    #[test]
    fn maps_unknown_geometry_types_to_unknown() {
        assert_eq!(GeometryType::from(""), GeometryType::Unknown);
        assert_eq!(GeometryType::from("CIRCULARSTRING"), GeometryType::Unknown);
        assert_eq!(GeometryType::from("point"), GeometryType::Unknown); // case-sensitive
    }

    #[test]
    fn holders_with_identical_state_are_equal() {
        let mut attributes = HashMap::new();
        attributes.insert("name".to_string(), "vienna".to_string());

        let a = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            false,
            Some(1),
            attributes.clone(),
        ));
        let b = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            false,
            Some(1),
            attributes,
        ));

        assert!(a == b);
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    #[test]
    fn holders_differing_in_geometry_type_are_not_equal() {
        let a = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            false,
            Some(1),
            HashMap::new(),
        ));
        let b = DynFeatureHolder(mock_feature(
            GeometryType::Line,
            false,
            Some(1),
            HashMap::new(),
        ));

        assert!(a != b);
    }

    #[test]
    fn holders_differing_in_attributes_are_not_equal() {
        let mut attributes = HashMap::new();
        attributes.insert("name".to_string(), "vienna".to_string());

        let a = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            false,
            Some(1),
            attributes,
        ));
        let b = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            false,
            Some(1),
            HashMap::new(),
        ));

        assert!(a != b);
    }

    #[test]
    fn holders_differing_in_deleted_flag_are_not_equal() {
        let a = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            false,
            Some(1),
            HashMap::new(),
        ));
        let b = DynFeatureHolder(mock_feature(
            GeometryType::Point,
            true,
            Some(1),
            HashMap::new(),
        ));

        assert!(a != b);
    }
}
