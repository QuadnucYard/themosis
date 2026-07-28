use std::ops::Range;

use thiserror::Error;

/// Stable identity for one source within a compilation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(u32);

impl SourceId {
    /// Creates an identity from its compilation-local index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the compilation-local index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// A half-open byte range in a source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    source: SourceId,
    start: usize,
    end: usize,
}

impl Span {
    /// Creates a span after checking that the range is ordered.
    pub fn new(source: SourceId, range: Range<usize>) -> Result<Self, InvalidSpan> {
        if range.end < range.start {
            return Err(InvalidSpan {
                start: range.start,
                end: range.end,
            });
        }

        Ok(Self {
            source,
            start: range.start,
            end: range.end,
        })
    }

    /// Returns the source containing this span.
    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    /// Returns the first byte included in this span.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the first byte after this span.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Returns the span length in bytes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    /// Returns whether the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Returns the half-open byte range.
    #[must_use]
    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

/// Error returned when a span ends before it starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("span end {end} is before start {start}")]
pub struct InvalidSpan {
    start: usize,
    end: usize,
}

impl InvalidSpan {
    /// Returns the invalid start offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the invalid end offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceId, Span};

    #[test]
    fn accepts_empty_and_non_empty_ranges() {
        let source = SourceId::new(7);
        let empty = Span::new(source, 2..2).expect("empty spans are valid");
        let populated = Span::new(source, 2..5).expect("ordered spans are valid");

        assert!(empty.is_empty());
        assert_eq!(populated.source(), source);
        assert_eq!(populated.range(), 2..5);
        assert_eq!(populated.len(), 3);
    }

    #[test]
    fn rejects_reversed_ranges() {
        #[expect(clippy::reversed_empty_ranges)]
        let range = 9..4;
        let error = Span::new(SourceId::new(0), range).expect_err("range is reversed");

        assert_eq!(error.start(), 9);
        assert_eq!(error.end(), 4);
        assert_eq!(error.to_string(), "span end 4 is before start 9");
    }
}
