use std::{error::Error, fmt};

/// A user-facing error with a stable diagnostic code.
pub trait Diagnostic: Error {
    /// Returns the stable diagnostic code for this error.
    fn code(&self) -> &str;
}

/// User-facing diagnostics collected in deterministic order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Errors<E: Diagnostic> {
    errors: Vec<E>,
}

impl<E: Diagnostic> Errors<E> {
    /// Creates a collection containing one diagnostic.
    #[must_use]
    pub fn one(error: E) -> Self {
        Self {
            errors: vec![error],
        }
    }

    /// Creates a collection from diagnostics already in presentation order.
    #[must_use]
    pub fn new(errors: Vec<E>) -> Self {
        Self { errors }
    }

    /// Returns the collected diagnostics.
    #[must_use]
    pub fn errors(&self) -> &[E] {
        &self.errors
    }

    /// Consumes the collection and returns its diagnostics.
    #[must_use]
    pub fn into_errors(self) -> Vec<E> {
        self.errors
    }

    /// Returns the number of collected diagnostics.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.errors.len()
    }

    /// Returns whether no diagnostics were collected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl<E: Diagnostic> fmt::Display for Errors<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "error[{}]: {error}", error.code())?;
        }
        Ok(())
    }
}

impl<E: Diagnostic> Error for Errors<E> {}

#[cfg(test)]
mod tests {
    use std::fmt;

    use super::{Diagnostic, Errors};

    #[derive(Debug)]
    struct TestDiagnostic(&'static str);

    impl fmt::Display for TestDiagnostic {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl std::error::Error for TestDiagnostic {}

    impl Diagnostic for TestDiagnostic {
        fn code(&self) -> &str {
            "TMS0001"
        }
    }

    #[test]
    fn formats_diagnostics_with_codes_and_line_breaks() {
        let errors = Errors::new(vec![TestDiagnostic("first"), TestDiagnostic("second")]);

        assert_eq!(
            errors.to_string(),
            "error[TMS0001]: first\nerror[TMS0001]: second"
        );
    }

    #[test]
    fn formats_empty_collection_as_empty_output() {
        assert_eq!(Errors::<TestDiagnostic>::new(Vec::new()).to_string(), "");
    }
}
