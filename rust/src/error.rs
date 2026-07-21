use gdal::errors::GdalError;
use thiserror::Error;

/// Crate-wide error type for fallible Geodot operations.
///
/// Wraps the underlying GDAL errors and adds variants for failures that
/// originate on the Godot/plugin side rather than from GDAL itself.
#[derive(Debug, Error)]
pub enum GeodotError {
    /// An error surfaced by the underlying GDAL library.
    #[error("GDAL error: {0}")]
    Gdal(#[from] GdalError),

    /// A Godot `Image` could not be created from the raster data.
    #[error("could not create Godot image: {0}")]
    ImageCreation(String),

    /// The raster/band format is not supported by the requested operation.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
}

#[cfg(test)]
mod tests {
    use super::GeodotError;
    use gdal::errors::GdalError;

    #[test]
    fn formats_image_creation_error() {
        let err = GeodotError::ImageCreation("bad data".to_string());
        assert_eq!(err.to_string(), "could not create Godot image: bad data");
    }

    #[test]
    fn formats_unsupported_format_error() {
        let err = GeodotError::UnsupportedFormat("Mixed".to_string());
        assert_eq!(err.to_string(), "unsupported format: Mixed");
    }

    #[test]
    fn converts_from_gdal_error() {
        let gdal_err = GdalError::BadArgument("boom".to_string());
        let err: GeodotError = gdal_err.into();
        assert!(matches!(err, GeodotError::Gdal(_)));
        assert!(err.to_string().starts_with("GDAL error:"));
    }
}
