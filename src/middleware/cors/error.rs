use std::fmt;

/// CORS configuration error
///
/// Returned by `CorsMiddlewareBuilder::build()` when the configuration
/// is invalid or violates CORS specification requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsConfigError {
    /// Wildcard origin (`*`) cannot be used with credentials
    ///
    /// This violates the CORS specification. When `allow_credentials` is `true`,
    /// you must specify exact origins, not a wildcard.
    WildcardWithCredentials,
    /// Invalid origin format
    ///
    /// The origin string does not match the expected format (scheme://host:port).
    InvalidOriginFormat {
        /// The invalid origin string
        origin: String,
    },
    /// Empty origins list with credentials
    ///
    /// When `allow_credentials` is `true`, at least one origin must be specified.
    EmptyOriginsWithCredentials,
}

impl fmt::Display for CorsConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorsConfigError::WildcardWithCredentials => {
                write!(
                    f,
                    "CORS configuration error: Cannot use wildcard origin (*) with credentials. \
                    When allow_credentials is true, you must specify exact origins."
                )
            }
            CorsConfigError::InvalidOriginFormat { origin } => {
                write!(
                    f,
                    "CORS configuration error: Invalid origin format '{}'. \
                    Expected format: scheme://host:port (e.g., https://example.com)",
                    origin
                )
            }
            CorsConfigError::EmptyOriginsWithCredentials => {
                write!(
                    f,
                    "CORS configuration error: Cannot use credentials with empty origins list. \
                    When allow_credentials is true, at least one origin must be specified."
                )
            }
        }
    }
}

impl std::error::Error for CorsConfigError {}

