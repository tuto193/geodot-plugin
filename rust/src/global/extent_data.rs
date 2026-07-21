use gdal::vector::Envelope as OGREnvelope;
#[derive(Default)]
pub struct ExtentData {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub down: f64,
}

impl ExtentData {
    pub fn from_envelope(envelope: OGREnvelope) -> Self {
        Self {
            left: envelope.MinX,
            right: envelope.MaxX,
            // TODO: check if top and down need to be switched
            top: envelope.MinY,
            down: envelope.MaxY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_envelope_bounds() {
        let envelope = OGREnvelope {
            MinX: 1.0,
            MaxX: 2.0,
            MinY: 3.0,
            MaxY: 4.0,
        };
        let extent = ExtentData::from_envelope(envelope);
        assert_eq!(extent.left, 1.0);
        assert_eq!(extent.right, 2.0);
        assert_eq!(extent.top, 3.0);
        assert_eq!(extent.down, 4.0);
    }

    #[test]
    fn default_is_zeroed() {
        let extent = ExtentData::default();
        assert_eq!(extent.left, 0.0);
        assert_eq!(extent.right, 0.0);
        assert_eq!(extent.top, 0.0);
        assert_eq!(extent.down, 0.0);
    }
}
