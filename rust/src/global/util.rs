use std::fmt;
pub struct Coords2D(pub f64, pub f64);

impl fmt::Display for Coords2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

#[cfg(test)]
mod tests {
    use super::Coords2D;

    #[test]
    fn display_is_space_separated() {
        assert_eq!(Coords2D(1.5, 2.5).to_string(), "1.5 2.5");
    }

    #[test]
    fn display_handles_negative_and_integer_values() {
        assert_eq!(Coords2D(-3.0, 4.0).to_string(), "-3 4");
    }
}
