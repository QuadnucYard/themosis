use std::cmp::Ordering;

use thiserror::Error;

/// Finite number with a canonical representation for negative zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Number(f64);

impl Number {
    /// Creates a finite number.
    pub fn new(value: f64) -> Result<Self, InvalidNumber> {
        if !value.is_finite() {
            return Err(InvalidNumber);
        }

        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    /// Returns the numeric value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Eq for Number {}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Error returned when a number is NaN or infinite.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("number must be finite")]
pub struct InvalidNumber;

/// An sRGB color with components in the inclusive range `0..=1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    components: [Number; 3],
    alpha: Number,
}

impl Color {
    /// Creates an sRGB color from red, green, blue, and alpha components.
    pub fn new(components: [f64; 3], alpha: f64) -> Result<Self, InvalidColor> {
        let [red, green, blue] = components;
        let components = [
            color_component("red", red)?,
            color_component("green", green)?,
            color_component("blue", blue)?,
        ];
        let alpha = color_component("alpha", alpha)?;

        Ok(Self { components, alpha })
    }

    /// Returns the red, green, and blue components.
    #[must_use]
    pub const fn components(self) -> [Number; 3] {
        self.components
    }

    /// Returns the alpha component.
    #[must_use]
    pub const fn alpha(self) -> Number {
        self.alpha
    }
}

fn color_component(name: &'static str, value: f64) -> Result<Number, InvalidColor> {
    let value = Number::new(value).map_err(|_| InvalidColor { name, value })?;
    if !(0.0..=1.0).contains(&value.get()) {
        return Err(InvalidColor {
            name,
            value: value.get(),
        });
    }

    Ok(value)
}

/// Error returned for a non-finite or out-of-range color component.
#[derive(Clone, Copy, Debug, PartialEq, Error)]
#[error("color component {name} must be finite and between 0 and 1, got {value}")]
pub struct InvalidColor {
    name: &'static str,
    value: f64,
}

impl InvalidColor {
    /// Returns the invalid component name.
    #[must_use]
    pub const fn component(self) -> &'static str {
        self.name
    }

    /// Returns the invalid component value.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }
}

/// Unit supported by a dimension token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionUnit {
    /// Pixel units interpreted according to backend policy.
    Pixel,
    /// Root-em units, retained for later compilation policy.
    Rem,
}

/// Numeric design-token dimension and its unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dimension {
    value: Number,
    unit: DimensionUnit,
}

impl Dimension {
    /// Creates a dimension with a finite numeric value.
    pub fn new(value: f64, unit: DimensionUnit) -> Result<Self, InvalidNumber> {
        Ok(Self {
            value: Number::new(value)?,
            unit,
        })
    }

    /// Returns the dimension magnitude.
    #[must_use]
    pub const fn value(self) -> Number {
        self.value
    }

    /// Returns the dimension unit.
    #[must_use]
    pub const fn unit(self) -> DimensionUnit {
        self.unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_negative_zero() {
        assert_eq!(
            Number::new(-0.0).expect("number is finite"),
            Number::new(0.0).expect("number is finite")
        );
    }

    #[test]
    fn rejects_invalid_colors() {
        let error = Color::new([1.1, 0.0, 0.0], 1.0).expect_err("red is out of range");

        assert_eq!(error.component(), "red");
        assert_eq!(error.value(), 1.1);
    }
}
