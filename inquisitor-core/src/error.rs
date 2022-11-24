/// Error type for this library
#[derive(Debug)]
pub enum InquisitorError {
    DurationParseError,
}

impl std::fmt::Display for InquisitorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::DurationParseError => write!(f, ""),
        }
    }
}

impl std::error::Error for InquisitorError {}
