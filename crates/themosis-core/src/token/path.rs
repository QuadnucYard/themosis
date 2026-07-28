use std::{fmt, str::FromStr};

use thiserror::Error;

/// Dot-separated path identifying a design token.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenPath {
    segments: Vec<String>,
}

impl TokenPath {
    /// Builds a path from individual segments.
    pub fn new<I, S>(segments: I) -> Result<Self, InvalidTokenPath>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let segments: Vec<String> = segments.into_iter().map(Into::into).collect();

        if segments.is_empty() {
            return Err(InvalidTokenPath::Empty);
        }

        if let Some(index) = segments.iter().position(String::is_empty) {
            return Err(InvalidTokenPath::EmptySegment { index });
        }

        Ok(Self { segments })
    }

    /// Iterates over path segments from group to token name.
    pub fn segments(&self) -> impl ExactSizeIterator<Item = &str> {
        self.segments.iter().map(String::as_str)
    }

    /// Returns the final token name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.segments
            .last()
            .expect("validated token paths always have a segment")
    }
}

impl FromStr for TokenPath {
    type Err = InvalidTokenPath;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(InvalidTokenPath::Empty);
        }

        Self::new(value.split('.'))
    }
}

impl fmt::Display for TokenPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut segments = self.segments();
        let first = segments
            .next()
            .expect("validated token paths always have a segment");
        formatter.write_str(first)?;

        for segment in segments {
            formatter.write_str(".")?;
            formatter.write_str(segment)?;
        }

        Ok(())
    }
}

/// Error returned when a token path has no usable segments.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum InvalidTokenPath {
    /// The path contained no segments.
    #[error("token path is empty")]
    Empty,
    /// A dot or supplied empty string created an empty segment.
    #[error("token path segment {index} is empty")]
    EmptySegment {
        /// Zero-based position of the empty segment.
        index: usize,
    },
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{InvalidTokenPath, TokenPath};

    #[test]
    fn round_trips_a_grouped_path() {
        let path = TokenPath::from_str("color.brand.primary").expect("path is valid");

        assert_eq!(
            path.segments().collect::<Vec<_>>(),
            ["color", "brand", "primary"]
        );
        assert_eq!(path.name(), "primary");
        assert_eq!(path.to_string(), "color.brand.primary");
    }

    #[test]
    fn rejects_empty_paths_and_segments() {
        assert_eq!(
            TokenPath::from_str("").expect_err("path is empty"),
            InvalidTokenPath::Empty
        );
        assert_eq!(
            TokenPath::from_str("color..primary").expect_err("segment is empty"),
            InvalidTokenPath::EmptySegment { index: 1 }
        );
    }

    #[test]
    fn sorts_paths_lexicographically_by_segment() {
        let mut paths = [
            TokenPath::from_str("space.large").expect("path is valid"),
            TokenPath::from_str("color.primary").expect("path is valid"),
            TokenPath::from_str("space.small").expect("path is valid"),
        ];

        paths.sort();

        assert_eq!(
            paths.map(|path| path.to_string()),
            ["color.primary", "space.large", "space.small"]
        );
    }
}
